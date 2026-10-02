# Testing the native CLI on Apple Silicon

This guide is for testing the `rust-port` bookmark alongside an existing Deno installation. The native CLI keeps version `3.0.0-alpha.1`. Use a separate executable named `linear-rust`; leave your existing `linear` and its credentials intact.

The user has already reported successful Apple Silicon checks on Rust 1.93: the native CLI read the existing Deno Keychain credential with the same token hash; auth list/whoami and issue mine/list/view/url/title, cycle/project/label/team list text and supported JSON matched. Their observed startup times were 24 ms native and 148 ms Deno. Those are manual checks on that machine, not a general performance guarantee. The earlier user-supplied log recorded 11 full-suite failures (18 reported); those historical results are retained. On corrected `rust-port` commit `fb5d3a4f7b542eb9de49338e1163d1b185b29951`, [native CI](https://github.com/schpet/linear-cli/actions/runs/37051334494) and [source CI](https://github.com/schpet/linear-cli/actions/runs/37051334806) both succeeded, with all nine required jobs GREEN. The required full Rust suites now pass on hosted macOS (952 passes/0 failures/1 ignored) and Linux (992 passes/0 failures/1 ignored), excluding nested filtered child-helper summaries. Native reader and source keyring integration checks pass on all three OSes. The optional Windows full suite still has101 failures across five targets and is not a required acceptance gate. These CI results do not replace a recheck on your own Mac; the steps below remain useful for manual Keychain, terminal and workspace testing.

## Get the source and toolchain

Install Apple's command-line tools if absent (`xcode-select --install`), [mise](https://mise.jdx.dev/installing-mise.html), and jj using your usual package manager. mise's [Rust backend](https://mise.jdx.dev/lang/rust.html) uses rustup. Install the pinned toolchain and components:

```sh
mise install rust@1.93.0
rustup toolchain install 1.93.0 --profile minimal --component clippy --component rustfmt
rustc +1.93.0 -vV
```

On an Apple Silicon native terminal, the host should be `aarch64-apple-darwin`. Record a different host, including a Rosetta `x86_64-apple-darwin` session, in your report.

For a new checkout:

```sh
jj git clone https://github.com/schpet/linear-cli.git linear-cli
cd linear-cli
jj git fetch --remote origin --branch rust-port
jj bookmark track rust-port@origin
jj new rust-port@origin
jj log -r '@ | @-' --no-graph
```

These commands leave an empty scratch change above the remote bookmark. In an existing checkout, inspect `jj status` and preserve your work before moving to another parent. See the [jj bookmark documentation](https://github.com/jj-vcs/jj/blob/main/docs/bookmarks.md). Do not edit a described/bookmarked change or move `main` for this test.

## Build and install a separate binary

Record the existing Deno command before building:

```sh
command -v linear
linear --version
cd rust
CARGO_BUILD_JOBS=2 cargo +1.93.0 build --locked --release -p linear-cli --bin linear
./target/release/linear --version
./target/release/linear --help
mkdir -p "$HOME/.local/bin"
```

Choose a new path. The following refuses to overwrite an existing `linear-rust`:

```sh
if test -e "$HOME/.local/bin/linear-rust"; then
  echo 'linear-rust already exists; choose another test filename' >&2
else
  install -m 755 target/release/linear "$HOME/.local/bin/linear-rust"
fi
```

Invoke that absolute path or add `$HOME/.local/bin` to your shell's PATH. This does not replace `linear`, run `cargo install --force`, or install a Deno shim. Record `shasum -a 256 "$HOME/.local/bin/linear-rust"` and `linear-rust --version`.

## Run Rust checks

The full suite needs Deno 2.7.9 on PATH because its transport tests run a cached-only synthetic server. Install that version with your usual tool manager, then run this from the repository root before testing:

```sh
deno --version
deno cache --frozen --config rust/parity/deno.json rust/parity/runner/serve-case.ts
```

Fish is optional for manual testing, but install it with `brew install fish` to exercise the real fish completion tests. CI installs fish on both Unix runners and bubblewrap on Linux; it does not rely on skipped fish tests.

From `rust/`, run the real macOS tests, not a Linux cross-target substitute:

```sh
CARGO_BUILD_JOBS=2 cargo +1.93.0 check --locked --workspace --all-targets
CARGO_BUILD_JOBS=2 cargo +1.93.0 clippy --locked -p linear-cli --all-targets -- -D warnings
cargo +1.93.0 fmt --all -- --check
CARGO_BUILD_JOBS=2 cargo +1.93.0 test --locked --no-fail-fast
```

If disk space is limited, invocation-only `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, and `CARGO_PROFILE_TEST_DEBUG=0` reduce build output without disabling debug assertions. Keep the original failure log; do not skip failing tests and call the full suite passed.

The CI reader checks can also be run separately:

```sh
cargo +1.93.0 test --locked -p linear-cli --test auth native_reader_spec
cargo +1.93.0 test --locked -p linear-cli --test auth windows
cargo +1.93.0 test --locked -p linear-cli --test commands native_reader_startup
cargo +1.93.0 test --locked -p linear-cli --test auth keyring
```

These use synthetic metadata, process fakes, and Windows blob contracts. They do not prove access to a real macOS Keychain or a real Windows Credential Manager. Do not opt into external-service integration tests using your everyday credentials.

## Read-only checks with an existing credential

First inspect `linear-rust auth --help` and each command's help. `auth list` and `auth whoami` read credentials/profile information; `auth token` prints the secret itself. Avoid saving or sharing token output. For a local hash comparison, pipe each implementation directly into `shasum -a 256`, never `tee` or a transcript.

`LINEAR_API_KEY` overrides saved credentials. A global `--workspace SLUG` selects a saved workspace and cannot be combined with that environment override. To test saved Keychain credentials, use a subshell so your original shell environment is restored:

```sh
(
  unset LINEAR_API_KEY
  linear-rust auth list
  linear-rust auth whoami
  linear-rust auth token | shasum -a 256
)
```

Run this only when reading your existing profile is intended. Neither the guide nor CI needs to inspect, copy, or print a credentials file.

For a workspace tour, use a known team key and IDs returned by the read commands themselves:

```sh
linear-rust team list --json
linear-rust issue query --team YOUR_TEAM --assignee @me --limit 1 --json
linear-rust issue view YOUR_ISSUE_ID --json --no-comments --no-download
linear-rust issue url YOUR_ISSUE_ID
linear-rust issue title YOUR_ISSUE_ID
linear-rust project list --team YOUR_TEAM --json
linear-rust cycle list --help
linear-rust label list --help
linear-rust issue mine --limit 1 --no-pager
```

Replace the placeholders; do not paste them literally. `issue mine` has no `--json` option. Consult each command's `--help` for `--no-pager` support; project list does not accept it. Avoid `--web`/`--app` when measuring CLI output. Issue view can download Markdown images unless `--no-download` is used; the example avoids that local file effect. Reads can reveal private workspace data, so keep captured output private.

## Optional credential-write checks

The rest of this section is **optional manual testing**. Login/logout/migrate change local credential metadata and may write/delete Keychain entries. Login performs a Linear API read to identify the workspace; these auth commands do not create Linear issues or other workspace objects. Do not run them on your everyday profile just to follow this guide.

Use a dedicated macOS test user with a disposable workspace credential for the strongest isolation. A temporary `XDG_CONFIG_HOME` isolates the CLI's metadata but **does not isolate the macOS Keychain**. Native Keychain entries use service `linear-cli` and the actual workspace slug as account; login upserts that entry and can overwrite the existing credential. Global `--workspace` is a selector, not a new independent Keychain profile. If you cannot establish that the account is disposable, skip credential writes.

In that dedicated environment, create a private neutral directory and metadata root, without copying your existing credentials:

```sh
AUTH_TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/linear-rust-auth.XXXXXX")
chmod 700 "$AUTH_TEST_ROOT"
AUTH_TEST_BINARY=$(command -v linear-rust)
linear_auth_test() (
  export XDG_CONFIG_HOME="$AUTH_TEST_ROOT/config"
  export LINEAR_IGNORE_ENV_FILE=1
  unset LINEAR_API_KEY
  cd "$AUTH_TEST_ROOT" || exit
  "$AUTH_TEST_BINARY" "$@"
)
linear_auth_test auth login --help
linear_auth_test auth logout --help
linear_auth_test auth migrate --help
```

Use the `linear_auth_test` wrapper for every optional command below; it reapplies the isolated metadata environment for each invocation. It still uses the current macOS user’s real Keychain, so the dedicated-user/disposable-account requirement remains. Without `--key`, `auth login` prompts for the secret; prefer that over putting a key in argv, shell history, or a failure report. The supported login options are `--key KEY` and `--plaintext`. `auth migrate` has no flags.

- `linear_auth_test auth login`: validate a disposable key, save it to Keychain and metadata, then inspect `auth list`/`auth whoami`. Cancelling a later prompt can leave an already saved credential; cancellation does not promise rollback.
- `linear_auth_test auth login --plaintext`: save plaintext to the isolated metadata file. Decline migration unless that file contains only disposable entries. Protect the directory and never include this file in a report.
- `linear_auth_test auth migrate`: move plaintext entries from that isolated file to Keychain. It handles all entries in the file and can leave partial writes on failure; do not run it against your normal metadata.
- `linear_auth_test auth logout DISPOSABLE_WORKSPACE_SLUG`: confirm deletion of that specific test credential. `--force` bypasses confirmation and is appropriate only after checking the disposable account identity. Do not use an unqualified logout or delete unrelated Keychain entries during cleanup.

After targeted cleanup, verify the disposable entry is gone using `linear_auth_test auth list`. Remove only the temporary directory you created after preserving non-secret logs. Never delete your normal `~/.config/linear` or existing Keychain accounts. This guide's automated Linux/foreign-cfg checks do not run these real-service commands.

## Terminal, editor, pager, and completions

Use a real terminal to check arrow-key selection, Unicode editing, masked secret entry, Ctrl-C, and restoration of echo/cursor after exit. Test stdin/stdout pipe topologies separately. Some source-measured commands explicitly refuse stdin-terminal/stdout-FIFO prompts; `CI=1` does not authorize a different API or write effect. A pipe EOF result is not proof of terminal restoration.

`linear-rust config` is an interactive read plus a **local `.linear.toml` write** on completion. Use a private neutral directory if you test it; cancelling before completion avoids that file write. It is not a read-only command to run in a real checkout without considering local configuration.

Pager/Markdown rendering, hyperlinks, and spinners depend on TTY/color settings. Compare ordinary terminal and `NO_COLOR=1` output, then supported `--no-pager` and redirected output. A nonempty `EDITOR` selects the editor; otherwise the CLI consults the configured Git editor. `PAGER` selects the pager when paging is supported and the output is long enough. Set these to harmless test programs in a disposable directory to check argv, inherited streams, and failures without opening applications. Fake pager/editor/opener regression tests can exercise wiring without editing real issues. To test an actual editor or issue/project write manually, use an explicitly disposable Linear test workspace, opt in separately, and review the command's help before submission. Those writes are not required by this guide.

The generated completion command supports the separate executable name:

```sh
# bash
source <(linear-rust completions bash --name linear-rust)

# zsh (after your normal completion initialization)
autoload -Uz compinit
compinit
source <(linear-rust completions zsh --name linear-rust)
```

For fish:

```fish
linear-rust completions fish --name linear-rust | source
```

Test completion in each shell you actually have, including the native `completions complete` callback through the generated grammar. Do not source scripts for absent shells and infer success, or replace your existing Deno completion configuration without reviewing it.

## Compare Deno and native behavior

Use the same known workspace/team/issue, explicit flags, environment, and color mode. Capture supported JSON to private files and compare with `cmp`; JSON preserves the API's field names, nesting, and pagination connection shape. Do not flatten connections to make output look equal. Script stdout and API/file effects remain exact gates for qualified source-success inputs.

The native parser is clap. Help, completion grammar, parser errors, and precisely recorded strict-input/terminal rendering boundaries differ from Cliffy; consult [compatibility.md](compatibility.md) rather than treating every stderr difference as approved. A source-success change to requests, mutation input, files, or script output still needs investigation. Time startup in the same shell/cache conditions and record sample counts; the reported 24/148 ms observation is not a threshold.

## Report a failure safely

Record macOS version/architecture, `rustc +1.93.0 -vV`, jj revision, binary SHA-256/version, exact non-secret arguments, TTY/pipe topology, exit status, and whether any local or API write had already occurred. Attach the failing test name and small stdout/stderr excerpts with tokens, authorization headers, credential files, personal/workspace payloads, and secret argv redacted. Retain the complete log privately. Label cross-compilation, fake-process tests, real Keychain tests, and real API reads separately; none substitutes for the others.
