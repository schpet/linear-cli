# auth

> Log in to workspaces and manage their credentials

## Usage

```
Log in to workspaces and manage their credentials

Usage: linear auth [OPTIONS] <COMMAND>

Commands:
  login    Log in to a workspace with an API key
  logout   Remove a workspace's credential
  list     List the workspaces you are logged in to
  default  Set the workspace used when none is named
  token    Print the API key in use
  whoami   Show who you are logged in as
  migrate  Move API keys from the credentials file to the system keyring
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### login

> Log in to a workspace with an API key

```
Log in to a workspace with an API key

Usage: linear auth login [OPTIONS]

Options:
  -k, --key <KEY>  API key; asked for, or read from stdin when it is piped
      --plaintext  Store the API key in the credentials file instead of the system keyring
  -h, --help       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### logout

> Remove a workspace's credential

```
Remove a workspace's credential

Usage: linear auth logout [OPTIONS] [WORKSPACE]

Arguments:
  [WORKSPACE]  Workspace to log out of [default: --workspace, or asked for when several are stored]

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### list

> List the workspaces you are logged in to

```
List the workspaces you are logged in to

Usage: linear auth list [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### default

> Set the workspace used when none is named

```
Set the workspace used when none is named

Usage: linear auth default [OPTIONS] [WORKSPACE]

Arguments:
  [WORKSPACE]  Workspace to make the default [default: --workspace, or asked for]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### token

> Print the API key in use

```
Print the API key in use

Usage: linear auth token [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### whoami

> Show who you are logged in as

```
Show who you are logged in as

Usage: linear auth whoami [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### migrate

> Move API keys from the credentials file to the system keyring

```
Move API keys from the credentials file to the system keyring

Usage: linear auth migrate [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
