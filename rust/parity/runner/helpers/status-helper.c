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
 * Protocol (parity-status/1): every frame is a 2-byte big-endian length
 * followed by at most FRAME_MAX ASCII bytes.
 *   helper -> runner  "parity-status/1 HELLO <nonce> <identity> <helperPid>"
 *   runner -> helper  "parity-status/1 ACK <nonce>"
 *   helper -> runner  "parity-status/1 RESULT <nonce> <identity> <childPid> exited <code>"
 *                  |  "parity-status/1 RESULT <nonce> <identity> <childPid> signaled <num> <NAME>"
 *                  |  "parity-status/1 RESULT <nonce> <identity> <childPid> exec-failed <errno>"
 * then EOF. The helper never writes to stdout, and writes to stderr only for
 * its own setup failures (prefix "status-helper: ").
 *
 * Pre-fork / harness exit codes (never a target result; the runner sees no
 * HELLO or no RESULT alongside them):
 *   120 usage        121 socket/connect      122 protocol (HELLO/ACK)
 *   123 fork         124 target exec failed  125 unrecognized wait status
 *
 * Invariants: no signal handlers, mask or umask changes; the only FDs the
 * helper opens are the CLOEXEC status socket and a CLOEXEC exec-error pipe,
 * and the child closes the socket explicitly before execve. The nonce is only
 * a correlation token, not a secret: bwrap PID 1's /proc/1/cmdline still
 * exposes it even after this helper scrubs its own argv storage.
 */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define PROTOCOL "parity-status/1"
#define FRAME_MAX 256
#define TOKEN_HEX 32
#define ACK_TIMEOUT_SEC 10

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
  /* Setup diagnostics only; a running target never shares this path. */
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

int main(int argc, char **argv) {
  if (argc < 5) fail(EXIT_USAGE, "usage: port nonce identity target [argv...]");
  uint16_t port;
  if (!parse_port(argv[1], &port)) fail(EXIT_USAGE, "invalid port");
  if (!is_hex_token(argv[2])) fail(EXIT_USAGE, "invalid nonce");
  if (!is_hex_token(argv[3])) fail(EXIT_USAGE, "invalid identity");
  if (argv[4][0] != '/') fail(EXIT_USAGE, "target must be an absolute path");

  char nonce[TOKEN_HEX + 1];
  char identity[TOKEN_HEX + 1];
  memcpy(nonce, argv[2], sizeof nonce);
  memcpy(identity, argv[3], sizeof identity);
  /* Correlation nonce only, but keep it out of /proc/<helper>/cmdline. */
  memset(argv[2], 'x', TOKEN_HEX);

  const char *target = argv[4];
  char **target_argv = argv + 4; /* target_argv[0] is the target path */

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
  int n = snprintf(frame, sizeof frame, PROTOCOL " HELLO %s %s %ld", nonce,
                   identity, (long)getpid());
  if (n <= 0 || (size_t)n >= sizeof frame) fail(EXIT_PROTOCOL, "hello format");
  if (send_frame(sock, frame) != 0) fail(EXIT_PROTOCOL, "hello send");

  char expected_ack[FRAME_MAX + 1];
  n = snprintf(expected_ack, sizeof expected_ack, PROTOCOL " ACK %s", nonce);
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

  /* CLOEXEC pipe: a successful execve closes it (EOF); a failure sends errno. */
  int exec_pipe[2];
  if (pipe2(exec_pipe, O_CLOEXEC) != 0) fail(EXIT_FORK, "pipe2");

  pid_t child = fork();
  if (child < 0) fail(EXIT_FORK, "fork");
  if (child == 0) {
    close(exec_pipe[0]);
    close(sock); /* CLOEXEC would do it; closing explicitly is the contract. */
    execve(target, target_argv, environ);
    int err = errno;
    ssize_t ignored = write(exec_pipe[1], &err, sizeof err);
    (void)ignored;
    _exit(127);
  }
  close(exec_pipe[1]);

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
    n = snprintf(frame, sizeof frame, PROTOCOL " RESULT %s %s %ld exited %d",
                 nonce, identity, (long)child, exit_code);
  } else if (WIFSIGNALED(status)) {
    int sig = WTERMSIG(status);
    if (sig < 1 || sig > SIGNAL_TABLE_MAX) {
      errno = 0;
      fail(EXIT_WAIT_STATUS, "signal outside the pinned table");
    }
    exit_code = 128 + sig;
    n = snprintf(frame, sizeof frame, PROTOCOL " RESULT %s %s %ld signaled %d %s",
                 nonce, identity, (long)child, sig, SIGNAL_NAMES[sig]);
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
