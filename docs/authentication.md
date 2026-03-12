# authentication

the CLI supports multiple authentication methods with the following precedence:

1. `--api-key` flag (explicit key for single command)
2. `LINEAR_API_KEY` environment variable
3. `api_key` in project `.linear.toml` config
4. `--workspace` / `-w` flag → stored credentials lookup (API key or managed relay)
5. project's `workspace` config → stored credentials lookup (API key or managed relay)
6. managed relay environment (`LINEAR_RELAY_BASE_URL` + `LINEAR_RELAY_ACCOUNT_ID`)
7. default workspace from stored credentials

## stored credentials (recommended)

API keys are stored in your system's native keyring (macOS Keychain, Linux libsecret, Windows CredentialManager). workspace metadata, including managed relay bindings, is stored in `~/.config/linear/credentials.toml`.

### commands

```bash
linear auth login              # add a workspace (prompts for API key)
linear auth login --key <key>  # add with key directly (for scripts)
linear auth login --managed --account-id <id> --relay-base-url <url>
linear auth list               # list configured workspaces
linear auth default            # interactively set default workspace
linear auth default <slug>     # set default workspace directly
linear auth logout <slug>      # remove a workspace
linear auth logout <slug> -f   # remove without confirmation
linear auth whoami             # show current user and workspace
linear auth token              # print the resolved API key
```

### adding workspaces

```bash
# first workspace becomes the default
$ linear auth login
Enter your Linear API key: ***
Logged in to workspace: Acme Corp (acme)
  User: Jane Developer <jane@acme.com>
  Set as default workspace

# add additional workspaces
$ linear auth login
Enter your Linear API key: ***
Logged in to workspace: Side Project (side-project)
  User: Jane Developer <jane@example.com>
```

### listing workspaces

```bash
$ linear auth list
  WORKSPACE    ORG NAME      USER
* acme         Acme Corp     Jane Developer <jane@acme.com>
  side-project Side Project  Jane Developer <jane@example.com>
```

the `*` indicates the default workspace.

### managed relay auth (mTLS)

for sandboxed/managed environments, the CLI can skip local API keys and authenticate through a relay that trusts your mTLS sandbox identity. this is designed for setups similar to the google-cli and slack managed flows.

required environment:

```bash
export LINEAR_MTLS_SHARED_SECRET="..."
export LINEAR_SANDBOX_ID="sandbox-123"
```

to log in and persist the relay binding locally:

```bash
linear auth login \
  --managed \
  --account-id acc-123 \
  --relay-base-url https://relay.example
```

or set the binding via env for ephemeral sessions:

```bash
export LINEAR_RELAY_BASE_URL="https://relay.example"
export LINEAR_RELAY_ACCOUNT_ID="acc-123"
```

managed auth stores the workspace slug, relay base URL, and account id locally, but it does **not** store an API key. `linear auth token` therefore only works for API-key workspaces.

### switching workspaces

```bash
# set a new default
linear auth default side-project

# or use -w flag for a single command
linear -w side-project issue list
linear -w acme issue create --title "Bug fix"
```

### credentials file format

```toml
# ~/.config/linear/credentials.toml
default = "acme"
workspaces = ["acme", "side-project"]

[managed.acme]
account_id = "acc-123"
relay_base_url = "https://relay.example"
```

API keys are not stored in this file. they are stored in the system keyring and loaded at startup. managed relay bindings are stored here because they only contain routing metadata, not secrets.

### platform requirements

- **macOS**: uses Keychain via `/usr/bin/security` (built-in)
- **Linux**: requires `secret-tool` from libsecret
  - Debian/Ubuntu: `apt install libsecret-tools`
  - Arch: `pacman -S libsecret`
- **Windows**: uses Credential Manager via `advapi32.dll` (built-in)

if the keyring is unavailable, set `LINEAR_API_KEY` as a fallback.

### migrating from plaintext credentials

older versions stored API keys directly in the TOML file. if the CLI detects this format, it will continue to work but print a warning. run `linear auth login` for each workspace to migrate keys to the system keyring.

## environment variable

for simpler setups or CI environments, you can use an environment variable:

```sh
# bash/zsh
export LINEAR_API_KEY="lin_api_..."

# fish
set -Ux LINEAR_API_KEY "lin_api_..."
```

this takes precedence over stored credentials. if you have `LINEAR_API_KEY` set and try to use `linear auth login`, you'll see a warning:

```
Warning: LINEAR_API_KEY environment variable is set.
It takes precedence over stored credentials.
Remove it from your shell config to use multi-workspace auth.
```

## project config

you can also set the API key in a project's `.linear.toml`:

```toml
api_key = "lin_api_..."
workspace = "acme"
team_id = "ENG"
```

this is useful for project-specific credentials but less secure than stored credentials since it may be committed to version control.

## workspace matching

when your project config has a `workspace` setting:

```toml
# .linear.toml
workspace = "acme"
team_id = "ENG"
```

the CLI will automatically use the stored credentials for that workspace, even if a different workspace is your default. this lets you work on multiple projects with different workspaces without constantly switching.

## creating an API key

1. go to [linear.app/settings/account/security](https://linear.app/settings/account/security)
2. scroll to "Personal API keys"
3. click "Create key"
4. give it a label (e.g., "CLI")
5. copy the key (starts with `lin_api_`)

note: creating an API key requires member access; it is not available for guest accounts.
