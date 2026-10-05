# api

> Send a raw GraphQL request to the Linear API

## Usage

```
Send a raw GraphQL request to the Linear API

Pass the GraphQL document as one quoted argument or on stdin. A leading `query` or `mutation`
keyword belongs inside that document.

Usage: linear api [OPTIONS] [DOCUMENT]

Arguments:
  [DOCUMENT]
          GraphQL query or mutation; read from stdin when omitted

Options:
      --variable <KEY=VALUE>
          Set a variable; repeatable
          
          A value that looks like a boolean, number, or null is sent as one, and `@path` reads the
          value from a file.

      --variables-json <JSON>
          Variables as a JSON object; --variable takes precedence

      --paginate
          Follow the cursor of the one connection in the response and print every page

      --silent
          Print nothing; the exit status still reports errors

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
