#![cfg(target_os = "linux")]

use crate::hydrate;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "linear-auth-local-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("config/linear")).unwrap();
        fs::create_dir_all(path.join("bin")).unwrap();
        Self(path)
    }
    fn credentials(&self, text: &str) {
        fs::write(self.path(), text).unwrap();
    }
    fn path(&self) -> PathBuf {
        self.0.join("config/linear/credentials.toml")
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }
    fn command(&self, leaf: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("APPDATA", self.0.join("config"))
            .env("PATH", self.0.join("bin"))
            .env("NO_COLOR", "1")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .args(["auth", leaf]);
        command
    }
    fn backend(&self, ending: &str) {
        let stub = self.0.join("bin/secret-tool");
        fs::write(&stub,format!("#!/bin/sh\nif [ \"$#\" -ne 5 ] || [ \"$1\" != lookup ] || [ \"$2\" != service ] || [ \"$3\" != linear-cli ] || [ \"$4\" != account ]; then exit 97; fi\ncase \"$5\" in zeta|alpha) ;; *) exit 98;; esac\nprintf '%s\\n' \"$@\" >> \"$5.argv\"\n{ending}\n")).unwrap();
        fs::set_permissions(stub, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn exactly_one_lookup(&self, name: &str) {
        assert_eq!(
            fs::read(self.0.join(format!("{name}.argv"))).unwrap(),
            format!("lookup\nservice\nlinear-cli\naccount\n{name}\n").as_bytes()
        );
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn run(mut command: Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.take(65537).read_to_end(&mut bytes).unwrap();
        bytes
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded fake auth command timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert!(
        stdout.len() <= 65536 && stderr.len() <= 65536,
        "fake auth output cap"
    );
    Output {
        status,
        stdout,
        stderr,
    }
}
const INLINE: &str =
    "default = \"zeta\"\nzeta = \"lin_api_fake_zeta\"\nalpha = \"lin_api_fake_alpha\"\n";
const META: &str = "default = \"zeta\"\nworkspaces = [\"zeta\",\"alpha\"]\n";

#[test]
fn token_preserves_raw_bytes_and_backend_terminal_lf_with_no_header_or_http_preparation() {
    let sandbox = Sandbox::new();
    sandbox.credentials(INLINE);
    let mut command = sandbox.command("token");
    command.env("LINEAR_API_KEY", " \tlin_api_fake_raw\r\nβ ");
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, " \tlin_api_fake_raw\r\nβ \n".as_bytes());
    assert!(output.stderr.is_empty());
    assert_eq!(sandbox.bytes(), INLINE.as_bytes());
    sandbox.credentials(META);
    sandbox.backend("printf 'lin_api_fake_backend_%s\\n' \"$5\"; exit 0");
    let output = run(sandbox.command("token"));
    assert!(output.status.success());
    assert_eq!(output.stdout, b"lin_api_fake_backend_zeta\n\n");
    sandbox.exactly_one_lookup("zeta");
    assert!(
        !sandbox.0.join("alpha.argv").exists(),
        "only the selected key is read"
    );
    assert_eq!(sandbox.bytes(), META.as_bytes());
}
#[test]
fn an_env_or_project_key_reads_no_keyring_entry() {
    for project in [false, true] {
        let sandbox = Sandbox::new();
        sandbox.credentials(META);
        sandbox.backend("exit 1");
        let mut command = sandbox.command("token");
        if project {
            fs::write(
                sandbox.0.join(".linear.toml"),
                "api_key='lin_api_fake_project'\n",
            )
            .unwrap();
        } else {
            command.env("LINEAR_API_KEY", "lin_api_fake_raw");
        }
        let output = run(command);
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            if project {
                b"lin_api_fake_project\n".to_vec()
            } else {
                b"lin_api_fake_raw\n".to_vec()
            }
        );
        assert!(output.stderr.is_empty());
        assert!(!sandbox.0.join("zeta.argv").exists());
        assert!(!sandbox.0.join("alpha.argv").exists());
        assert_eq!(sandbox.bytes(), META.as_bytes());
    }
}
#[test]
fn token_precedence_empty_env_and_project_key_before_missing_explicit_workspace() {
    let sandbox = Sandbox::new();
    sandbox.credentials(INLINE);
    fs::write(
        sandbox.0.join(".linear.toml"),
        "api_key='lin_api_fake_project'\nworkspace='alpha'\n",
    )
    .unwrap();
    let mut command = sandbox.command("token");
    command.args(["--workspace", "ghost"]);
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"lin_api_fake_project\n");
    let mut command = sandbox.command("token");
    command.env("LINEAR_API_KEY", "");
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"lin_api_fake_alpha\n");
    fs::write(
        sandbox.0.join(".linear.toml"),
        "api_key='lin_api_fake_project'\n",
    )
    .unwrap();
    let mut command = sandbox.command("token");
    command.env("LINEAR_API_KEY", "");
    assert_eq!(run(command).stdout, b"lin_api_fake_zeta\n");
}
#[test]
fn token_exact_absent_conflict_and_missing_explicit_errors_never_print_a_secret() {
    let sandbox = Sandbox::new();
    let output = run(sandbox.command("token"));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to get API token: No API key configured\n  Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.\n");
    sandbox.credentials(INLINE);
    let mut command = sandbox.command("token");
    command
        .env("LINEAR_API_KEY", "lin_api_fake_secret")
        .args(["--workspace", "alpha"]);
    let output = run(command);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to get API token: Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.\n");
    let mut command = sandbox.command("token");
    command.args(["--workspace", "ghost"]);
    let output = run(command);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to get API token: Workspace \"ghost\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces.\n");
}
#[test]
fn both_leaves_inherit_strict_startup_rejection_including_raw_token_and_source_permissive_rewrite()
{
    for leaf in ["token", "default"] {
        let sandbox = Sandbox::new();
        let text =
            "default='zeta'\nzeta='lin_api_fake_zeta'\nalpha='lin_api_fake_alpha'\nunrelated=7\n";
        sandbox.credentials(text);
        sandbox.backend("exit 97");
        let mut command = sandbox.command(leaf);
        command.env("LINEAR_API_KEY", "lin_api_fake_raw");
        if leaf == "default" {
            command.arg("alpha");
        }
        let output = run(command);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("invalid value type")
        );
        assert_eq!(sandbox.bytes(), text.as_bytes());
        assert!(!sandbox.0.join("alpha.argv").exists());
        assert!(!sandbox.0.join("zeta.argv").exists());
    }
}
#[test]
fn default_zero_single_invalid_argument_and_current_target_keep_exact_early_returns_and_no_writes()
{
    let sandbox = Sandbox::new();
    let mut command = sandbox.command("default");
    command.arg("ghost");
    let output = run(command);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to set default workspace: No workspaces configured\n  Run `linear auth login` to add a workspace\n");
    let text = "solo='lin_api_fake_solo'\n";
    sandbox.credentials(text);
    for arg in [None, Some("ghost"), Some("")] {
        let mut command = sandbox.command("default");
        if let Some(arg) = arg {
            command.arg(arg);
        }
        let output = run(command);
        assert!(output.status.success());
        assert_eq!(output.stdout, b"Only one workspace configured: solo\n");
        assert!(output.stderr.is_empty());
        assert_eq!(sandbox.bytes(), text.as_bytes());
    }
    sandbox.credentials(INLINE);
    let mut command = sandbox.command("default");
    command.arg("zeta");
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"\"zeta\" is already the default workspace\n"
    );
    assert_eq!(sandbox.bytes(), INLINE.as_bytes());
}
#[test]
fn default_saves_inline_exactly_preserves_mode_and_next_token_reloads_the_new_default() {
    let sandbox = Sandbox::new();
    sandbox.credentials(INLINE);
    fs::set_permissions(sandbox.path(), fs::Permissions::from_mode(0o600)).unwrap();
    let mut command = sandbox.command("default");
    command
        .arg("alpha")
        .env("LINEAR_API_KEY", "lin_api_fake_ignored")
        .args(["--workspace", "ghost"]);
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Default workspace set to: alpha\n");
    assert!(output.stderr.is_empty());
    assert_eq!(
        sandbox.bytes(),
        b"default = \"alpha\"\nalpha = \"lin_api_fake_alpha\"\nzeta = \"lin_api_fake_zeta\"\n"
    );
    assert_eq!(
        fs::metadata(sandbox.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let output = run(sandbox.command("token"));
    assert!(output.status.success());
    assert_eq!(output.stdout, b"lin_api_fake_alpha\n");
}
#[test]
fn metadata_default_save_does_not_require_keys_and_never_stores_or_deletes_backend_entries() {
    let sandbox = Sandbox::new();
    sandbox.credentials(META);
    sandbox.backend("exit 1");
    let mut command = sandbox.command("default");
    command.arg("alpha");
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Default workspace set to: alpha\n");
    assert_eq!(
        sandbox.bytes(),
        b"default = \"alpha\"\nworkspaces = [\"alpha\", \"zeta\"]\n"
    );
    sandbox.exactly_one_lookup("zeta");
    sandbox.exactly_one_lookup("alpha");
}
#[test]
fn default_membership_is_exact_ordered_and_non_tty_selection_refuses_before_menu_or_write() {
    let sandbox = Sandbox::new();
    sandbox.credentials(INLINE);
    let mut command = sandbox.command("default");
    command.arg("ghost");
    let output = run(command);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to set default workspace: Workspace not found: ghost\n  Available workspaces: zeta, alpha\n");
    for arg in [None, Some("")] {
        let mut command = sandbox.command("default");
        command.env("CI", "1");
        if let Some(arg) = arg {
            command.arg(arg);
        }
        let output = run(command);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr,b"\xe2\x9c\x97 Failed to set default workspace: A workspace is required when stdin is not a terminal\n  Specify a workspace with `linear auth default <workspace>`.\n");
        assert_eq!(sandbox.bytes(), INLINE.as_bytes());
    }
}
#[test]
fn menu_data_is_typed_before_session_but_explicit_whitespace_save_remains_exact() {
    use linear_cli::auth::parse_credentials;
    use linear_cli::commands::auth::default::{DefaultAction, prepare};
    use linear_cli::config::{RawConfigFile, parse_config_tier};
    for name in [" ", "\t"] {
        let text = format!(
            "default='alpha'\nalpha='lin_api_fake_alpha'\n{}='lin_api_fake_unusual'\n",
            serde_json::to_string(name).unwrap()
        );
        let manifest = parse_credentials(
            parse_config_tier(RawConfigFile {
                path: PathBuf::from("/fake/store"),
                bytes: text.as_bytes().to_vec(),
            })
            .unwrap(),
        )
        .unwrap();
        let store = hydrate(manifest, vec![]).unwrap();
        let error = prepare(&store, None).unwrap_err();
        assert!(error.message().contains("cannot be selected interactively"));
        assert!(matches!(
            prepare(&store, Some(name)).unwrap(),
            DefaultAction::Save(_)
        ));
    }
    let sandbox = Sandbox::new();
    sandbox
        .credentials("default='alpha'\nalpha='lin_api_fake_alpha'\n\" \"='lin_api_fake_space'\n");
    let mut command = sandbox.command("default");
    command.arg(" ");
    let output = run(command);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Default workspace set to:  \n");
    assert_eq!(
        sandbox.bytes(),
        b"default = \" \"\n\" \" = \"lin_api_fake_space\"\nalpha = \"lin_api_fake_alpha\"\n"
    );
}
#[test]
fn native_help_extra_positionals_and_unsupported_json_preserve_usage_boundaries() {
    let sandbox = Sandbox::new();
    for leaf in ["token", "default"] {
        let mut command = sandbox.command(leaf);
        command.arg("--help");
        let output = run(command);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
        let mut command = sandbox.command(leaf);
        command.arg("--json");
        assert_eq!(run(command).status.code(), Some(2));
    }
    let mut command = sandbox.command("token");
    command.arg("extra");
    assert_eq!(run(command).status.code(), Some(2));
    let mut command = sandbox.command("default");
    command.args(["alpha", "extra"]);
    assert_eq!(run(command).status.code(), Some(2));
}

#[test]
fn prepared_menu_keeps_first_row_highlight_and_current_label_and_matches_arrow_choice() {
    use linear_cli::auth::parse_credentials;
    use linear_cli::commands::auth::default::{DefaultAction, prepare};
    use linear_cli::config::{RawConfigFile, parse_config_tier};
    use linear_cli::platform::prompt::{PlainSelect, PromptKey, PromptOutcome, PromptSession};
    let text = "default='alpha'\nzeta='lin_api_fake_zeta'\nalpha='lin_api_fake_alpha'\n";
    let store = hydrate(
        parse_credentials(
            parse_config_tier(RawConfigFile {
                path: PathBuf::from("/fake/store"),
                bytes: text.as_bytes().to_vec(),
            })
            .unwrap(),
        )
        .unwrap(),
        vec![],
    )
    .unwrap();
    let DefaultAction::Select(options) = prepare(&store, None).unwrap() else {
        panic!("prompt required");
    };
    assert_eq!(options[0].value, "zeta");
    assert_eq!(options[1].label, "alpha (current)");
    assert!(
        options
            .iter()
            .all(|option| option.script_token == option.value)
    );
    for (keys, expected) in [
        (
            vec![PromptKey::Enter],
            PromptOutcome::Submitted("zeta".to_owned()),
        ),
        (
            vec![PromptKey::Down, PromptKey::Enter],
            PromptOutcome::Submitted("alpha".to_owned()),
        ),
        (vec![PromptKey::Interrupt], PromptOutcome::Interrupted),
    ] {
        let mut keys = keys.into_iter();
        let mut session =
            PromptSession::<std::io::Empty, Vec<u8>>::keys(Vec::new(), 80, 24, move || {
                Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
            })
            .unwrap();
        let result = session.select(&PlainSelect {
            message: "Select default workspace",
            options: &options,
            default_index: 0,
            default_hint: None,
        });
        assert_eq!(session.finish_result(result).unwrap(), expected);
    }
}
