/*
 * status-helper: authenticated exact exit-status observer for the parity lane.
 *
 * Bubblewrap's reaper folds a signal death into exit code 128+n, so the
 * runner cannot tell `exit(143)` from SIGTERM. This helper is the bwrap
 * command instead of the target. It connects to a runner-owned loopback
 * listener BEFORE forking, proves its identity with a per-run nonce, waits
 * for the runner's ACK, then forks once, execs the exact target with the
 * original argv/env/cwd/streams, waitpid()s that one child and reports the
 * raw wait status over the socket. It exits with the target's code, or
 * 128+n for a signal, so bwrap's outer status stays consistent.
 *
 * Protocol (parity-status/2): every frame is a 2-byte big-endian length
 * followed by at most FRAME_MAX ASCII bytes.
 *   helper -> runner  "parity-status/2 HELLO <nonce> <identity> <helperPid>"
 *   runner -> helper  "parity-status/2 ACK <nonce>"
 *   helper -> runner  "parity-status/2 RESULT <nonce> <identity> <childPid> exited <code> stdout <mode> <count> <relayed> <closure>"
 *                  |  "parity-status/2 RESULT <nonce> <identity> <childPid> signaled <num> <NAME> stdout <mode> <count> <relayed> <closure>"
 *                  |  "parity-status/2 RESULT <nonce> <identity> <childPid> exec-failed <errno>"
 * then EOF. In pipe modes the helper relays target bytes to its stdout.
 * Its setup/relay failures write prefixed diagnostics to shared stderr.
 *
 * Prototype PTY mode (parity-status/3) leaves the /2 pipe path untouched.
 * Its argv mode is `all-pty` with a `columnsxrows` size in the count slot.
 * It creates a PTY inside bwrap, makes the exact target a session leader and
 * controlling-tty foreground group, and relays the master to helper stdout.
 * Helper stdin accepts canonical `ACTION <index> raw|cooked <key-hex>\n` lines.
 * After exec confirmation, a matching termios gate, a zero-byte drain to
 * EAGAIN and a second matching gate, the helper writes one key event and
 * reports `APPLIED <nonce> <index> <relayed-byte-count>` on the status socket.
 * RESULT at exact target reap carries requested size, relayed count, final
 * termios flags/class and observed winsize. `DONE <nonce> <total>` follows
 * only proven post-reap EIO. A retained slave instead yields authenticated
 * `RETAINED <nonce> <total>` after a bounded grace, then a target-consistent
 * outer exit. An internal relay guard emits `LIMIT <nonce> <total>` before
 * or after target reap and exits 126; a prior RESULT is valid. A short or blocked key
 * write emits `INPUT_FAILURE <nonce> <index> <relayed> <written> <errno>`
 * with the same early outcome. No /3 frame or helper diagnostic enters the
 * PTY transcript.
 *
 * Pre-fork / harness exit codes (never a target result; the runner sees no
 * HELLO or no RESULT alongside them):
 *   120 usage        121 socket/connect      122 protocol (HELLO/ACK)
 *   123 fork         124 target exec failed  125 unrecognized wait status
 *
 * Invariants: child signal disposition, mask and umask match A1; the child
 * closes the helper socket, exec-error pipe and private read end before execve.
 * The nonce is only
 * a correlation token, not a secret: bwrap PID 1's /proc/1/cmdline still
 * exposes it even after this helper scrubs its own argv storage.
 */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <pty.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/time.h>
#include <termios.h>
#include <time.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define PROTOCOL "parity-status/2"
#define FRAME_MAX 256
#define TOKEN_HEX 32
#define ACK_TIMEOUT_SEC 10
#define MAX_PIPE_BYTES (64u * 1024u * 1024u)

enum stdout_mode { MODE_DRAIN, MODE_CLOSED, MODE_AFTER, MODE_PTY };

enum {
  EXIT_USAGE = 120,
  EXIT_SOCKET = 121,
  EXIT_PROTOCOL = 122,
  EXIT_FORK = 123,
  EXIT_EXEC_FAILED = 124,
  EXIT_WAIT_STATUS = 125,
};

extern char **environ;

/* Pinned Linux x86_64 signal names; checked against the platform macros. */
static const char *const SIGNAL_NAMES[] = {
    NULL,        "SIGHUP",  "SIGINT",    "SIGQUIT", "SIGILL",    "SIGTRAP",
    "SIGABRT",   "SIGBUS",  "SIGFPE",    "SIGKILL", "SIGUSR1",   "SIGSEGV",
    "SIGUSR2",   "SIGPIPE", "SIGALRM",   "SIGTERM", "SIGSTKFLT", "SIGCHLD",
    "SIGCONT",   "SIGSTOP", "SIGTSTP",   "SIGTTIN", "SIGTTOU",   "SIGURG",
    "SIGXCPU",   "SIGXFSZ", "SIGVTALRM", "SIGPROF", "SIGWINCH",  "SIGIO",
    "SIGPWR",    "SIGSYS",
};
#define SIGNAL_TABLE_MAX 31
#define LINUX_SIGNAL_MAX 64
_Static_assert(SIGHUP == 1 && SIGINT == 2 && SIGQUIT == 3 && SIGILL == 4,
               "signal table drift");
_Static_assert(SIGTRAP == 5 && SIGABRT == 6 && SIGBUS == 7 && SIGFPE == 8,
               "signal table drift");
_Static_assert(SIGKILL == 9 && SIGUSR1 == 10 && SIGSEGV == 11 && SIGUSR2 == 12,
               "signal table drift");
_Static_assert(SIGPIPE == 13 && SIGALRM == 14 && SIGTERM == 15 &&
                   SIGSTKFLT == 16,
               "signal table drift");
_Static_assert(SIGCHLD == 17 && SIGCONT == 18 && SIGSTOP == 19 && SIGTSTP == 20,
               "signal table drift");
_Static_assert(SIGTTIN == 21 && SIGTTOU == 22 && SIGURG == 23 && SIGXCPU == 24,
               "signal table drift");
_Static_assert(SIGXFSZ == 25 && SIGVTALRM == 26 && SIGPROF == 27 &&
                   SIGWINCH == 28,
               "signal table drift");
_Static_assert(SIGIO == 29 && SIGPWR == 30 && SIGSYS == 31,
               "signal table drift");

static void fail(int code, const char *what) {
  int saved = errno;
  const char *reason = saved == 0 ? "" : strerror(saved);
  /* Setup and relay failures share FD 2 with the target; the runner treats
   * this prefix without a valid RESULT as a harness error. */
  fprintf(stderr, "status-helper: %s%s%s\n", what, saved == 0 ? "" : ": ",
          reason);
  _exit(code);
}

static int is_hex_token(const char *s) {
  size_t n = strlen(s);
  if (n != TOKEN_HEX) return 0;
  for (size_t i = 0; i < n; i++) {
    char c = s[i];
    if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) return 0;
  }
  return 1;
}

static int parse_port(const char *s, uint16_t *out) {
  size_t n = strlen(s);
  if (n == 0 || n > 5) return 0;
  unsigned long value = 0;
  for (size_t i = 0; i < n; i++) {
    if (s[i] < '0' || s[i] > '9') return 0;
    value = value * 10 + (unsigned long)(s[i] - '0');
  }
  if (value == 0 || value > 65535) return 0;
  *out = (uint16_t)value;
  return 1;
}

static int parse_count(const char *s, unsigned int *out) {
  size_t n = strlen(s);
  if (n == 0 || n > 8 || (n > 1 && s[0] == '0')) return 0;
  unsigned int value = 0;
  for (size_t i = 0; i < n; i++) {
    if (s[i] < '0' || s[i] > '9') return 0;
    value = value * 10u + (unsigned int)(s[i] - '0');
  }
  if (value > MAX_PIPE_BYTES) return 0;
  *out = value;
  return 1;
}

static int parse_size(const char *s, unsigned short *columns,
                      unsigned short *rows) {
  unsigned int col = 0, row = 0;
  const char *p = s;
  if (*p < '1' || *p > '9') return 0;
  while (*p >= '0' && *p <= '9') {
    col = col * 10u + (unsigned int)(*p++ - '0');
    if (col > 500) return 0;
  }
  if (*p++ != 'x' || *p < '1' || *p > '9') return 0;
  while (*p >= '0' && *p <= '9') {
    row = row * 10u + (unsigned int)(*p++ - '0');
    if (row > 200) return 0;
  }
  if (*p != '\0') return 0;
  *columns = (unsigned short)col;
  *rows = (unsigned short)row;
  return 1;
}

static int relay_prefix(int fd, unsigned int count, unsigned int *relayed) {
  unsigned char buf[4096];
  while (*relayed < count) {
    size_t wanted = count - *relayed;
    if (wanted > sizeof buf) wanted = sizeof buf;
    ssize_t got = read(fd, buf, wanted);
    if (got < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    if (got == 0) return 0; /* threshold not reached */
    size_t sent = 0;
    while (sent < (size_t)got) {
      ssize_t wrote = write(STDOUT_FILENO, buf + sent, (size_t)got - sent);
      if (wrote < 0) {
        if (errno == EINTR) continue;
        return -1;
      }
      if (wrote == 0) {
        errno = EIO;
        return -1;
      }
      sent += (size_t)wrote;
    }
    *relayed += (unsigned int)got;
  }
  return 1;
}

static int write_all(int fd, const void *buf, size_t len) {
  const unsigned char *p = buf;
  while (len > 0) {
    ssize_t n = send(fd, p, len, MSG_NOSIGNAL);
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    p += n;
    len -= (size_t)n;
  }
  return 0;
}

static int read_all(int fd, void *buf, size_t len) {
  unsigned char *p = buf;
  while (len > 0) {
    ssize_t n = recv(fd, p, len, 0);
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    if (n == 0) {
      errno = 0;
      return -1;
    }
    p += n;
    len -= (size_t)n;
  }
  return 0;
}

static int send_frame(int fd, const char *payload) {
  size_t len = strlen(payload);
  if (len == 0 || len > FRAME_MAX) {
    errno = EMSGSIZE;
    return -1;
  }
  unsigned char header[2] = {(unsigned char)(len >> 8),
                             (unsigned char)(len & 0xff)};
  if (write_all(fd, header, sizeof header) != 0) return -1;
  return write_all(fd, payload, len);
}

/* Reads one frame into buf (NUL-terminated); returns -1 on EOF/error/oversize. */
static int recv_frame(int fd, char *buf, size_t cap) {
  unsigned char header[2];
  if (read_all(fd, header, sizeof header) != 0) return -1;
  size_t len = ((size_t)header[0] << 8) | header[1];
  if (len == 0 || len > FRAME_MAX || len >= cap) {
    errno = EMSGSIZE;
    return -1;
  }
  if (read_all(fd, buf, len) != 0) return -1;
  buf[len] = '\0';
  return 0;
}

/* PTY mode uses parity-status/3 while pipe modes keep their exact /2 wire
 * contract. The action control is a bounded line on helper stdin; it never
 * reaches the target's slave. */
#define PTY_PROTOCOL "parity-status/3"
#define PTY_ACTION_MAX 128
#define PTY_KEY_MAX 16
#define PTY_DRAIN_GRACE_MS 2000
#define PTY_DRAIN_PASS_BYTES (64u * 1024u)

static uint64_t now_ms(void) {
  struct timespec t;
  if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) fail(EXIT_PROTOCOL, "clock");
  return (uint64_t)t.tv_sec * 1000u + (uint64_t)t.tv_nsec / 1000000u;
}

static void pty_baseline(int slave) {
  struct termios term;
  memset(&term, 0, sizeof term);
  term.c_iflag = ICRNL;
  term.c_oflag = OPOST | ONLCR;
  term.c_cflag = CS8 | CREAD;
  term.c_lflag = ECHO | ICANON | ISIG | IEXTEN;
  term.c_cc[VINTR] = 3;
  term.c_cc[VQUIT] = 28;
  term.c_cc[VERASE] = 127;
  term.c_cc[VKILL] = 21;
  term.c_cc[VEOF] = 4;
  term.c_cc[VMIN] = 1;
  term.c_cc[VTIME] = 0;
  if (tcsetattr(slave, TCSANOW, &term) != 0) fail(EXIT_FORK, "pty tcsetattr");
}

enum drain_result { DRAIN_EMPTY, DRAIN_BUDGET, DRAIN_EOF, DRAIN_LIMIT };

static enum drain_result relay_master(int master, uint64_t *relayed) {
  unsigned char buf[4096];
  size_t pass_bytes = 0;
  while (pass_bytes < PTY_DRAIN_PASS_BYTES) {
    ssize_t got = read(master, buf, sizeof buf);
    if (got < 0) {
      if (errno == EINTR) continue;
      if (errno == EAGAIN || errno == EWOULDBLOCK) return DRAIN_EMPTY;
      /* Linux's PTY master returns EIO only after the last slave closes. */
      if (errno == EIO) return DRAIN_EOF;
      fail(EXIT_PROTOCOL, "pty master read");
    }
    if (got == 0) return DRAIN_EOF;
    size_t sent = 0;
    while (sent < (size_t)got) {
      ssize_t wrote = write(STDOUT_FILENO, buf + sent, (size_t)got - sent);
      if (wrote < 0 && errno == EINTR) continue;
      /* A runner-initiated cap/deadline kill may close its reader first. Its
       * own kill reason is authoritative; do not write a helper diagnostic. */
      if (wrote < 0 && errno == EPIPE) _exit(EXIT_PROTOCOL);
      if (wrote <= 0) fail(EXIT_PROTOCOL, "pty relay write");
      sent += (size_t)wrote;
    }
    *relayed += (uint64_t)got;
    pass_bytes += (size_t)got;
    if (*relayed > MAX_PIPE_BYTES) {
      return DRAIN_LIMIT;
    }
  }
  return DRAIN_BUDGET;
}

struct pty_action {
  unsigned int index;
  int raw;
  unsigned char key[PTY_KEY_MAX];
  size_t length;
};

static int hex_digit(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  return -1;
}

static int one_key_event(const unsigned char *key, size_t length) {
  if (length == 1) return key[0] < 0x80;
  if (length == 3 && key[0] == 0x1b && key[1] == '[' && key[2] != 0 &&
      strchr("ABCDHF", key[2]) != NULL) return 1;
  if (length == 4 && key[0] == 0x1b && key[1] == '[' &&
      key[2] == '3' && key[3] == '~') return 1;
  unsigned int code;
  if (length == 2 && key[0] >= 0xc2 && key[0] <= 0xdf &&
      (key[1] & 0xc0) == 0x80) return 1;
  if (length == 3 && key[0] >= 0xe0 && key[0] <= 0xef &&
      (key[1] & 0xc0) == 0x80 && (key[2] & 0xc0) == 0x80) {
    code = ((unsigned int)(key[0] & 0x0f) << 12) |
           ((unsigned int)(key[1] & 0x3f) << 6) | (key[2] & 0x3f);
    return code >= 0x800 && (code < 0xd800 || code > 0xdfff);
  }
  if (length == 4 && key[0] >= 0xf0 && key[0] <= 0xf4 &&
      (key[1] & 0xc0) == 0x80 && (key[2] & 0xc0) == 0x80 &&
      (key[3] & 0xc0) == 0x80) {
    code = ((unsigned int)(key[0] & 0x07) << 18) |
           ((unsigned int)(key[1] & 0x3f) << 12) |
           ((unsigned int)(key[2] & 0x3f) << 6) | (key[3] & 0x3f);
    return code >= 0x10000 && code <= 0x10ffff;
  }
  return 0;
}

static void parse_action(char *line, unsigned int expected,
                         struct pty_action *action) {
  unsigned int index = 0;
  char state[16] = {0}, hex[PTY_KEY_MAX * 2 + 2] = {0}, extra = 0;
  if (sscanf(line, "ACTION %u %15s %33s %c", &index, state, hex, &extra) != 3 ||
      index != expected ||
      (strcmp(state, "raw") != 0 && strcmp(state, "cooked") != 0)) {
    errno = 0;
    fail(EXIT_PROTOCOL, "invalid PTY action");
  }
  char canonical[PTY_ACTION_MAX + 1];
  int canonical_len = snprintf(canonical, sizeof canonical, "ACTION %u %s %s",
                               index, state, hex);
  if (canonical_len <= 0 || (size_t)canonical_len >= sizeof canonical ||
      strcmp(line, canonical) != 0) {
    errno = 0;
    fail(EXIT_PROTOCOL, "noncanonical PTY action");
  }
  size_t length = strlen(hex);
  if (length == 0 || length > PTY_KEY_MAX * 2 || length % 2 != 0) {
    errno = 0;
    fail(EXIT_PROTOCOL, "invalid PTY key length");
  }
  action->index = index;
  action->raw = strcmp(state, "raw") == 0;
  action->length = length / 2;
  for (size_t i = 0; i < action->length; i++) {
    int high = hex_digit(hex[i * 2]), low = hex_digit(hex[i * 2 + 1]);
    if (high < 0 || low < 0) {
      errno = 0;
      fail(EXIT_PROTOCOL, "invalid PTY key hex");
    }
    action->key[i] = (unsigned char)((high << 4) | low);
  }
  if (!one_key_event(action->key, action->length)) {
    errno = 0;
    fail(EXIT_PROTOCOL, "PTY action is not one key event");
  }
}

enum termios_state { TERM_OTHER, TERM_RAW, TERM_COOKED };

static enum termios_state pty_state(int master) {
  struct termios term;
  if (tcgetattr(master, &term) != 0) fail(EXIT_PROTOCOL, "pty tcgetattr");
  tcflag_t flags = term.c_lflag & (ICANON | ECHO | ISIG);
  if (flags == 0) return TERM_RAW;
  if (flags == (ICANON | ECHO | ISIG)) return TERM_COOKED;
  return TERM_OTHER;
}

static const char *pty_state_name(struct termios *term) {
  tcflag_t flags = term->c_lflag & (ICANON | ECHO | ISIG);
  if (flags == 0) return "raw";
  if (flags == (ICANON | ECHO | ISIG)) return "cooked";
  return "other";
}

static int target_exit_code(int status) {
  return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}

static int pty_limit(int sock, const char *nonce, uint64_t relayed,
                     int master, int exec_read_fd) {
  char frame[FRAME_MAX + 1];
  int n = snprintf(frame, sizeof frame, PTY_PROTOCOL " LIMIT %s %llu", nonce,
                   (unsigned long long)relayed);
  if (n > 0 && (size_t)n < sizeof frame && send_frame(sock, frame) == 0) {
    (void)shutdown(sock, SHUT_WR);
  }
  close(sock);
  close(master);
  close(exec_read_fd);
  /* This is an authenticated output-limit outcome, never a target exit.
   * Bubblewrap's PID-1 reaper tears down the target's separate session. */
  return 126;
}

static int pty_input_failure(int sock, const char *nonce,
                             const struct pty_action *action,
                             uint64_t relayed, ssize_t wrote, int write_errno,
                             int master, int exec_read_fd) {
  char frame[FRAME_MAX + 1];
  int n = snprintf(frame, sizeof frame,
                   PTY_PROTOCOL " INPUT_FAILURE %s %u %llu %zd %d", nonce,
                   action->index, (unsigned long long)relayed, wrote,
                   write_errno);
  if (n > 0 && (size_t)n < sizeof frame && send_frame(sock, frame) == 0) {
    (void)shutdown(sock, SHUT_WR);
  }
  close(sock);
  close(master);
  close(exec_read_fd);
  return 126;
}

static int run_pty(int sock, const char *nonce, const char *identity,
                   unsigned short columns, unsigned short rows,
                   const char *target, char **target_argv) {
  struct winsize size = {.ws_col = columns, .ws_row = rows};
  int master, slave;
  if (openpty(&master, &slave, NULL, NULL, &size) != 0) fail(EXIT_FORK, "openpty");
  pty_baseline(slave);
  int exec_pipe[2];
  if (pipe2(exec_pipe, O_CLOEXEC | O_NONBLOCK) != 0) fail(EXIT_FORK, "pty pipe2");
  pid_t child = fork();
  if (child < 0) fail(EXIT_FORK, "pty fork");
  if (child == 0) {
    close(master);
    close(sock);
    close(exec_pipe[0]);
    if (setsid() < 0 || ioctl(slave, TIOCSCTTY, 0) < 0 ||
        tcsetpgrp(slave, getpgrp()) < 0 ||
        dup2(slave, STDIN_FILENO) < 0 ||
        dup2(slave, STDOUT_FILENO) < 0 ||
        dup2(slave, STDERR_FILENO) < 0) {
      int setup_error = -errno;
      ssize_t ignored = write(exec_pipe[1], &setup_error, sizeof setup_error);
      (void)ignored;
      _exit(127);
    }
    if (slave > STDERR_FILENO) close(slave);
    /* The target sees only its controlling slave, never helper control FDs. */
    execve(target, target_argv, environ);
    int err = errno;
    ssize_t ignored = write(exec_pipe[1], &err, sizeof err);
    (void)ignored;
    _exit(127);
  }
  close(slave);
  close(exec_pipe[1]);
  struct sigaction ignore_pipe = {0};
  ignore_pipe.sa_handler = SIG_IGN;
  if (sigaction(SIGPIPE, &ignore_pipe, NULL) != 0) {
    fail(EXIT_PROTOCOL, "PTY SIGPIPE disposition");
  }
  if (fcntl(master, F_SETFL, fcntl(master, F_GETFL) | O_NONBLOCK) != 0 ||
      fcntl(STDIN_FILENO, F_SETFL, fcntl(STDIN_FILENO, F_GETFL) | O_NONBLOCK) != 0) {
    fail(EXIT_FORK, "pty nonblock");
  }
  char frame[FRAME_MAX + 1];
  char action_line[PTY_ACTION_MAX + 1];
  size_t action_used = 0;
  struct pty_action action = {0};
  int pending = 0, input_eof = 0, master_eof = 0, reaped = 0, status = 0;
  int exec_confirmed = 0, exec_report_read = 0;
  int exec_errno = 0;
  unsigned int next_index = 0;
  uint64_t relayed = 0, reaped_at = 0;
  for (;;) {
    /* A busy writer cannot monopolize this loop: each pass reads at most
     * 64 KiB before waitpid, action and grace checks run again. */
    enum drain_result drained = relay_master(master, &relayed);
    if (drained == DRAIN_LIMIT) {
      return pty_limit(sock, nonce, relayed, master, exec_pipe[0]);
    }
    if (reaped && drained == DRAIN_EOF) master_eof = 1;
    if (!exec_confirmed && !exec_report_read) {
      ssize_t exec_read = read(exec_pipe[0], &exec_errno, sizeof exec_errno);
      if (exec_read == 0) exec_confirmed = 1;
      else if (exec_read == (ssize_t)sizeof exec_errno) exec_report_read = 1;
      else if (exec_read < 0 && (errno == EAGAIN || errno == EINTR)) { /* not yet */ }
      else fail(EXIT_PROTOCOL, "PTY exec confirmation read");
    }
    if (!reaped) {
      pid_t waited = waitpid(child, &status, WNOHANG);
      if (waited < 0) fail(EXIT_WAIT_STATUS, "pty waitpid");
      if (waited == child) {
        reaped = 1;
        reaped_at = now_ms();
        /* Reap proves the CLOEXEC writer is closed, so errno cannot race
         * with the RESULT classification. */
        if (!exec_confirmed && !exec_report_read) {
          ssize_t exec_read;
          do {
            exec_read = read(exec_pipe[0], &exec_errno, sizeof exec_errno);
          } while (exec_read < 0 && errno == EINTR);
          if (exec_read == 0) exec_confirmed = 1;
          else if (exec_read == (ssize_t)sizeof exec_errno) exec_report_read = 1;
          else fail(EXIT_PROTOCOL, "PTY exec error read");
        }
        struct termios final_term;
        struct winsize final_size;
        if (tcgetattr(master, &final_term) != 0 ||
            ioctl(master, TIOCGWINSZ, &final_size) != 0) {
          fail(EXIT_PROTOCOL, "PTY final terminal sample");
        }
        int n;
        if (exec_errno < 0) {
          n = snprintf(frame, sizeof frame,
                       PTY_PROTOCOL " RESULT %s %s %ld setup-failed %d",
                       nonce, identity, (long)child, -exec_errno);
        } else if (exec_errno > 0) {
          n = snprintf(frame, sizeof frame,
                       PTY_PROTOCOL " RESULT %s %s %ld exec-failed %d",
                       nonce, identity, (long)child, exec_errno);
        } else if (WIFEXITED(status)) {
          n = snprintf(frame, sizeof frame,
                       PTY_PROTOCOL " RESULT %s %s %ld exited %d pty %u %u %llu %s %lu %lu %lu %u %u",
                       nonce, identity, (long)child, WEXITSTATUS(status), columns,
                       rows, (unsigned long long)relayed,
                       pty_state_name(&final_term), (unsigned long)final_term.c_iflag,
                       (unsigned long)final_term.c_oflag,
                       (unsigned long)final_term.c_lflag,
                       final_size.ws_col, final_size.ws_row);
        } else if (WIFSIGNALED(status) && WTERMSIG(status) <= SIGNAL_TABLE_MAX) {
          int sig = WTERMSIG(status);
          n = snprintf(frame, sizeof frame,
                       PTY_PROTOCOL " RESULT %s %s %ld signaled %d %s pty %u %u %llu %s %lu %lu %lu %u %u",
                       nonce, identity, (long)child, sig, SIGNAL_NAMES[sig], columns,
                       rows, (unsigned long long)relayed,
                       pty_state_name(&final_term), (unsigned long)final_term.c_iflag,
                       (unsigned long)final_term.c_oflag,
                       (unsigned long)final_term.c_lflag,
                       final_size.ws_col, final_size.ws_row);
        } else if (WIFSIGNALED(status) && WTERMSIG(status) <= LINUX_SIGNAL_MAX) {
          int sig = WTERMSIG(status);
          n = snprintf(frame, sizeof frame,
                       PTY_PROTOCOL " RESULT %s %s %ld signaled-unlisted %d pty %u %u %llu %s %lu %lu %lu %u %u",
                       nonce, identity, (long)child, sig, columns, rows,
                       (unsigned long long)relayed, pty_state_name(&final_term),
                       (unsigned long)final_term.c_iflag,
                       (unsigned long)final_term.c_oflag,
                       (unsigned long)final_term.c_lflag,
                       final_size.ws_col, final_size.ws_row);
        } else {
          errno = 0;
          fail(EXIT_WAIT_STATUS, "pty unrecognized wait status");
        }
        if (n <= 0 || (size_t)n >= sizeof frame || send_frame(sock, frame) != 0) {
          fail(EXIT_PROTOCOL, "pty result send");
        }
      }
    }
    /* EIO before reap is transient evidence only. A slave may open again;
     * final EOF must be observed by a fresh read after exact target reap. */
    if (reaped && !master_eof && drained == DRAIN_EOF) {
      enum drain_result final_probe = relay_master(master, &relayed);
      if (final_probe == DRAIN_LIMIT) {
        return pty_limit(sock, nonce, relayed, master, exec_pipe[0]);
      }
      master_eof = final_probe == DRAIN_EOF;
    }
    if (reaped && master_eof) break;
    if (reaped && now_ms() - reaped_at >= PTY_DRAIN_GRACE_MS) {
      int n = snprintf(frame, sizeof frame, PTY_PROTOCOL " RETAINED %s %llu",
                       nonce, (unsigned long long)relayed);
      if (n <= 0 || (size_t)n >= sizeof frame || send_frame(sock, frame) != 0 ||
          shutdown(sock, SHUT_WR) != 0) {
        fail(EXIT_PROTOCOL, "PTY retained send");
      }
      close(sock);
      close(master);
      close(exec_pipe[0]);
      return target_exit_code(status);
    }
    int boundary_eof = 0;
    if (pending && exec_confirmed && !reaped && drained != DRAIN_EOF &&
        pty_state(master) == (action.raw ? TERM_RAW : TERM_COOKED)) {
      /* Every byte before the key belongs to the old transcript boundary. */
      uint64_t before = relayed;
      enum drain_result boundary = relay_master(master, &relayed);
      if (boundary == DRAIN_LIMIT) {
        return pty_limit(sock, nonce, relayed, master, exec_pipe[0]);
      }
      if (boundary == DRAIN_EOF) boundary_eof = 1;
      /* Cliffy renders before entering raw. A zero-byte drain after the
       * first matching gate, then a second matching gate, puts the entire
       * render before APPLIED. A busy writer cannot create a false boundary. */
      if (boundary == DRAIN_EMPTY && relayed == before && pty_state(master) ==
          (action.raw ? TERM_RAW : TERM_COOKED)) {
        ssize_t wrote = write(master, action.key, action.length);
        if (wrote != (ssize_t)action.length) {
          int write_errno = wrote < 0 ? errno : 0;
          return pty_input_failure(sock, nonce, &action, relayed, wrote,
                                   write_errno, master, exec_pipe[0]);
        }
        int n = snprintf(frame, sizeof frame, PTY_PROTOCOL " APPLIED %s %u %llu",
                         nonce, action.index, (unsigned long long)relayed);
        if (n <= 0 || (size_t)n >= sizeof frame || send_frame(sock, frame) != 0) {
          fail(EXIT_PROTOCOL, "PTY action ACK send");
        }
        next_index++;
        pending = 0;
      }
    }
    struct pollfd fds[2] = {{.fd = master, .events = POLLIN},
                            {.fd = STDIN_FILENO, .events = POLLIN}};
    if (master_eof || (!reaped && (drained == DRAIN_EOF || boundary_eof))) {
      fds[0].fd = -1;
    }
    if (input_eof || pending || reaped) fds[1].fd = -1;
    int ready = poll(fds, 2, 20);
    if (ready < 0 && errno != EINTR) fail(EXIT_PROTOCOL, "PTY poll");
    if (ready > 0 && fds[1].revents != 0) {
      char c;
      ssize_t got = read(STDIN_FILENO, &c, 1);
      if (got < 0 && (errno == EAGAIN || errno == EINTR)) continue;
      if (got < 0) fail(EXIT_PROTOCOL, "PTY action read");
      if (got == 0) { input_eof = 1; continue; }
      if (c == '\0') {
        errno = 0;
        fail(EXIT_PROTOCOL, "NUL in PTY action");
      }
      if (c == '\n') {
        action_line[action_used] = '\0';
        parse_action(action_line, next_index, &action);
        pending = 1;
        action_used = 0;
      } else {
        if (action_used >= PTY_ACTION_MAX) {
          errno = EMSGSIZE;
          fail(EXIT_PROTOCOL, "PTY action too long");
        }
        action_line[action_used++] = c;
      }
    }
  }
  /* A target may exit while the runner is writing an action frame. With no
   * applied ACK, the live runner classifies that as an input mismatch. */
  int n = snprintf(frame, sizeof frame, PTY_PROTOCOL " DONE %s %llu", nonce,
                   (unsigned long long)relayed);
  if (n <= 0 || (size_t)n >= sizeof frame || send_frame(sock, frame) != 0) {
    fail(EXIT_PROTOCOL, "PTY done send");
  }
  close(master);
  close(exec_pipe[0]);
  if (shutdown(sock, SHUT_WR) != 0) fail(EXIT_PROTOCOL, "PTY shutdown");
  close(sock);
  if (exec_errno < 0) {
    errno = -exec_errno;
    fail(EXIT_FORK, "PTY child setup");
  }
  if (exec_errno > 0) {
    errno = exec_errno;
    fail(EXIT_EXEC_FAILED, "PTY execve");
  }
  return target_exit_code(status);
}

int main(int argc, char **argv) {
  if (argc < 7) fail(EXIT_USAGE, "usage: port nonce identity mode count target [argv...]");
  uint16_t port;
  if (!parse_port(argv[1], &port)) fail(EXIT_USAGE, "invalid port");
  if (!is_hex_token(argv[2])) fail(EXIT_USAGE, "invalid nonce");
  if (!is_hex_token(argv[3])) fail(EXIT_USAGE, "invalid identity");
  enum stdout_mode mode;
  if (strcmp(argv[4], "drain") == 0) mode = MODE_DRAIN;
  else if (strcmp(argv[4], "closed-at-start") == 0) mode = MODE_CLOSED;
  else if (strcmp(argv[4], "close-after-bytes") == 0) mode = MODE_AFTER;
  else if (strcmp(argv[4], "all-pty") == 0) mode = MODE_PTY;
  else fail(EXIT_USAGE, "invalid stdout mode");
  unsigned int count;
  unsigned short columns = 0, rows = 0;
  if (mode == MODE_PTY ? !parse_size(argv[5], &columns, &rows) :
      (!parse_count(argv[5], &count) ||
       (mode == MODE_AFTER ? count == 0 : count != 0))) {
    fail(EXIT_USAGE, "invalid stdout count");
  }
  if (mode == MODE_PTY) count = 0;
  if (argv[6][0] != '/') fail(EXIT_USAGE, "target must be an absolute path");

  char nonce[TOKEN_HEX + 1];
  char identity[TOKEN_HEX + 1];
  memcpy(nonce, argv[2], sizeof nonce);
  memcpy(identity, argv[3], sizeof identity);
  /* Correlation nonce only, but keep it out of /proc/<helper>/cmdline. */
  memset(argv[2], 'x', TOKEN_HEX);

  const char *target = argv[6];
  char **target_argv = argv + 6; /* target_argv[0] is the target path */

  int sock = socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC, 0);
  if (sock < 0) fail(EXIT_SOCKET, "socket");
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof addr);
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  if (inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr) != 1) {
    fail(EXIT_SOCKET, "inet_pton");
  }
  for (;;) {
    if (connect(sock, (struct sockaddr *)&addr, sizeof addr) == 0) break;
    if (errno == EINTR) continue;
    fail(EXIT_SOCKET, "connect");
  }
  struct timeval ack_timeout = {ACK_TIMEOUT_SEC, 0};
  if (setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &ack_timeout,
                 sizeof ack_timeout) != 0) {
    fail(EXIT_SOCKET, "setsockopt");
  }

  char frame[FRAME_MAX + 1];
  const char *protocol = mode == MODE_PTY ? PTY_PROTOCOL : PROTOCOL;
  int n = snprintf(frame, sizeof frame, "%s HELLO %s %s %ld", protocol, nonce,
                   identity, (long)getpid());
  if (n <= 0 || (size_t)n >= sizeof frame) fail(EXIT_PROTOCOL, "hello format");
  if (send_frame(sock, frame) != 0) fail(EXIT_PROTOCOL, "hello send");

  char expected_ack[FRAME_MAX + 1];
  n = snprintf(expected_ack, sizeof expected_ack, "%s ACK %s", protocol, nonce);
  if (n <= 0 || (size_t)n >= sizeof expected_ack) {
    fail(EXIT_PROTOCOL, "ack format");
  }
  if (recv_frame(sock, frame, sizeof frame) != 0) fail(EXIT_PROTOCOL, "ack read");
  if (strcmp(frame, expected_ack) != 0) {
    errno = 0;
    fail(EXIT_PROTOCOL, "ack mismatch");
  }
  /* The ACK deadline was for the handshake only; the target owns the clock now. */
  struct timeval no_timeout = {0, 0};
  if (setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &no_timeout,
                 sizeof no_timeout) != 0) {
    fail(EXIT_SOCKET, "setsockopt");
  }

  if (mode == MODE_PTY) {
    return run_pty(sock, nonce, identity, columns, rows, target, target_argv);
  }

  /* CLOEXEC pipe: a successful execve closes it (EOF); a failure sends errno. */
  int exec_pipe[2];
  if (pipe2(exec_pipe, O_CLOEXEC) != 0) fail(EXIT_FORK, "pipe2");

  int stdout_pipe[2] = {-1, -1};
  if (mode != MODE_DRAIN) {
    if (pipe2(stdout_pipe, O_CLOEXEC) != 0) fail(EXIT_FORK, "stdout pipe2");
    if (mode == MODE_CLOSED) {
      if (close(stdout_pipe[0]) != 0) fail(EXIT_FORK, "close private read");
      stdout_pipe[0] = -1;
    }
  }

  pid_t child = fork();
  if (child < 0) fail(EXIT_FORK, "fork");
  if (child == 0) {
    close(exec_pipe[0]);
    close(sock); /* CLOEXEC would do it; closing explicitly is the contract. */
    if (mode != MODE_DRAIN) {
      if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
      if (dup2(stdout_pipe[1], STDOUT_FILENO) < 0) {
        int err = errno;
        ssize_t ignored = write(exec_pipe[1], &err, sizeof err);
        (void)ignored;
        _exit(127);
      }
      close(stdout_pipe[1]);
    }
    execve(target, target_argv, environ);
    int err = errno;
    ssize_t ignored = write(exec_pipe[1], &err, sizeof err);
    (void)ignored;
    _exit(127);
  }
  close(exec_pipe[1]);
  if (mode != MODE_DRAIN && close(stdout_pipe[1]) != 0) {
    fail(EXIT_FORK, "close private write");
  }

  int exec_errno = 0;
  size_t got = 0;
  while (got < sizeof exec_errno) {
    ssize_t r = read(exec_pipe[0], (char *)&exec_errno + got,
                     sizeof exec_errno - got);
    if (r < 0) {
      if (errno == EINTR) continue;
      fail(EXIT_FORK, "exec pipe read");
    }
    if (r == 0) break;
    got += (size_t)r;
  }
  close(exec_pipe[0]);
  if (got != 0 && got != sizeof exec_errno) {
    errno = 0;
    fail(EXIT_FORK, "short exec pipe read");
  }

  if (got == sizeof exec_errno && stdout_pipe[0] >= 0) {
    close(stdout_pipe[0]);
    stdout_pipe[0] = -1;
  }

  unsigned int relayed = 0;
  const char *closure = "none";
  if (got == 0 && mode == MODE_CLOSED) closure = "before-start";
  if (got == 0 && mode == MODE_AFTER) {
    /* Change only the helper parent's disposition, after the target fork. */
    struct sigaction ignore = {0};
    ignore.sa_handler = SIG_IGN;
    if (sigaction(SIGPIPE, &ignore, NULL) != 0) fail(EXIT_PROTOCOL, "sigaction");
    int reached = relay_prefix(stdout_pipe[0], count, &relayed);
    if (close(stdout_pipe[0]) != 0) fail(EXIT_PROTOCOL, "close private read");
    stdout_pipe[0] = -1;
    if (reached < 0) fail(EXIT_PROTOCOL, "relay");
    closure = reached == 1 ? "after-N" : "threshold-not-reached";
  }

  int status;
  for (;;) {
    pid_t waited = waitpid(child, &status, 0);
    if (waited == child) break;
    if (waited < 0 && errno == EINTR) continue;
    fail(EXIT_WAIT_STATUS, "waitpid");
  }

  if (got == sizeof exec_errno) {
    n = snprintf(frame, sizeof frame, PROTOCOL " RESULT %s %s %ld exec-failed %d",
                 nonce, identity, (long)child, exec_errno);
    if (n <= 0 || (size_t)n >= sizeof frame) fail(EXIT_PROTOCOL, "result format");
    if (send_frame(sock, frame) != 0) fail(EXIT_PROTOCOL, "result send");
    close(sock);
    errno = exec_errno;
    fail(EXIT_EXEC_FAILED, "execve");
  }

  int exit_code;
  if (WIFEXITED(status)) {
    exit_code = WEXITSTATUS(status);
    n = snprintf(frame, sizeof frame,
                 PROTOCOL " RESULT %s %s %ld exited %d stdout %s %u %u %s",
                 nonce, identity, (long)child, exit_code, argv[4], count,
                 relayed, closure);
  } else if (WIFSIGNALED(status)) {
    int sig = WTERMSIG(status);
    if (sig < 1 || sig > SIGNAL_TABLE_MAX) {
      errno = 0;
      fail(EXIT_WAIT_STATUS, "signal outside the pinned table");
    }
    exit_code = 128 + sig;
    n = snprintf(frame, sizeof frame,
                 PROTOCOL " RESULT %s %s %ld signaled %d %s stdout %s %u %u %s",
                 nonce, identity, (long)child, sig, SIGNAL_NAMES[sig], argv[4],
                 count, relayed, closure);
  } else {
    errno = 0;
    fail(EXIT_WAIT_STATUS, "unrecognized wait status");
  }
  if (n <= 0 || (size_t)n >= sizeof frame) fail(EXIT_PROTOCOL, "result format");
  if (send_frame(sock, frame) != 0) fail(EXIT_PROTOCOL, "result send");
  if (shutdown(sock, SHUT_WR) != 0) fail(EXIT_PROTOCOL, "shutdown");
  close(sock);
  return exit_code;
}
