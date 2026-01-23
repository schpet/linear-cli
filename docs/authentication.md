# authentication

the CLI supports multiple authentication methods with the following precedence:

1. `--api-key` flag (explicit key for single command)
2. `LINEAR_API_KEY` environment variable
3. `api_key` in project `.linear.toml` config
4. `--workspace` / `-w` flag → stored credentials lookup
5. project's `workspace` config → stored credentials lookup
6. default workspace from stored credentials

## stored credentials (recommended)

credentials are stored in `~/.config/linear/credentials.toml` and support multiple workspaces.

### commands

```bash
linear auth login              # add a workspace (prompts for API key)
linear auth login --key <key>  # add with key directly (for scripts)
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
acme = "lin_api_xxx"
side-project = "lin_api_yyy"
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
