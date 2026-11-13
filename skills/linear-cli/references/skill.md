# skill

> Teach coding agents to use linear

## Usage

```
Teach coding agents to use linear

`linear skill show` prints a guide to this CLI written for coding agents. The guide ships in this
binary, so upgrading linear also updates what agents read.

Usage: linear skill [OPTIONS] <COMMAND>

Commands:
  show  Print the guide that teaches an agent to use linear
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

## Subcommands

### show

> Print the guide that teaches an agent to use linear

```
Print the guide that teaches an agent to use linear

The guide is Markdown and comes from this binary, so it describes the commands this version has. It
needs no login, configuration or network connection. Run `linear COMMAND --help` for a command's
full options.

Usage: linear skill show [OPTIONS]

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
