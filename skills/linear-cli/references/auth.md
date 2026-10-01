# auth

> Manage Linear authentication

```
Manage Linear authentication

Usage: linear auth [OPTIONS] [COMMAND]

Commands:
  login    Add a workspace credential
  logout   Remove a workspace credential
  list     List configured workspaces
  default  Set the default workspace
  token    Print the configured API token
  whoami   Print information about the authenticated user
  migrate  Migrate plaintext credentials to system keyring
  help     Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## login

> Add a workspace credential

```
Add a workspace credential

Usage: linear auth login [OPTIONS]

Options:
  -k, --key <key>         API key (prompted if not provided)
      --workspace <slug>  Target workspace (uses credentials)
      --plaintext         Store API key in credentials file instead of system keyring
  -h, --help              Print help
```

## logout

> Remove a workspace credential

```
Remove a workspace credential

Usage: linear auth logout [OPTIONS] [workspace]

Arguments:
  [workspace]  

Options:
  -f, --force             Skip confirmation prompt
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List configured workspaces

```
List configured workspaces

Usage: linear auth list [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## default

> Set the default workspace

```
Set the default workspace

Usage: linear auth default [OPTIONS] [workspace]

Arguments:
  [workspace]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## token

> Print the configured API token

```
Print the configured API token

Usage: linear auth token [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## whoami

> Print information about the authenticated user

```
Print information about the authenticated user

Usage: linear auth whoami [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## migrate

> Migrate plaintext credentials to system keyring

```
Migrate plaintext credentials to system keyring

Usage: linear auth migrate [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
