# authentication

the CLI picks the API key for a command in this order:

1. `LINEAR_API_KEY`, from the environment or a `.env` file. combining it with `--workspace` is an error, since the key already decides the workspace
2. `api_key` in a `.linear.toml` config file
3. the stored key of the workspace named by `--workspace`
4. the stored key of the configured `workspace` (`.linear.toml` or `LINEAR_WORKSPACE`)
5. the stored key of the default workspace in the credentials file

workspace selection is strict: once a workspace is chosen by `--workspace`, the `workspace` setting, or the default, its stored key must be usable. if it is missing (for example, its keyring entry was deleted), the command fails and says where the choice came from instead of falling back to another workspace's key.

## stored credentials (recommended)

API keys are stored in your system's native keyring (macOS Keychain, Linux libsecret, Windows Credential Manager). the list of workspaces and the default are stored in the credentials file:

- macOS and Linux: `$XDG_CONFIG_HOME/linear/credentials.toml`, or `~/.config/linear/credentials.toml` when `XDG_CONFIG_HOME` is unset
- Windows: `%APPDATA%\linear\credentials.toml`

### commands

```bash
linear auth login              # add a workspace (prompts for API key)
linear auth login --key <key>  # add with key directly (for scripts)
linear auth list               # list configured workspaces
linear auth default            # interactively set default workspace
linear auth default <slug>     # set default workspace directly
linear auth logout <slug>      # remove a workspace
linear auth logout <slug> --yes  # remove without confirmation
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
   WORKSPACE     ORGANIZATION  USER
*  acme          Acme Corp     Jane Developer <jane@acme.com>
   side-project  Side Project  Jane Developer <jane@example.com>
```

the `*` indicates the default workspace.

### switching workspaces

```bash
# set a new default
linear auth default side-project

# or use --workspace flag for a single command
linear --workspace side-project issue list
linear --workspace acme issue create --title "Bug fix"
```

### credentials file format

```toml
# credentials.toml
default = "acme"
workspaces = ["acme", "side-project"]
```

API keys are not stored in this file. they are stored in the system keyring and read when a command needs them, usually just the key of the selected workspace.

### platform requirements

- **macOS**: uses Keychain via `/usr/bin/security` (built-in)
- **Linux**: requires `secret-tool` from libsecret
  - Debian/Ubuntu: `apt install libsecret-tools`
  - Arch: `pacman -S libsecret`
- **Windows**: uses Credential Manager via `advapi32.dll` (built-in)

if the keyring is unavailable, set `LINEAR_API_KEY` as a fallback.

### migrating from plaintext credentials

older versions stored API keys directly in the credentials file, and `linear auth login --plaintext` still does. that format keeps working. to move every key into the system keyring, run:

```bash
linear auth migrate
```

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
Warning: LINEAR_API_KEY is set and takes precedence over stored credentials.
Remove it from your shell config to use multi-workspace auth.
```

## project config

you can also set the API key in a project's `.linear.toml`:

```toml
api_key = "lin_api_..."
workspace = "acme"
team_id = "ENG"
issue_create_assign_self = "always"
issue_create_ask_project = true
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
