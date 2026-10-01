# api

> Make a raw GraphQL API request

```
Make a raw GraphQL API request

Pass the GraphQL document as one quoted argument or on stdin. The api command has no subcommands: a leading query or mutation keyword belongs inside that document.

Usage: linear api [OPTIONS] [graphqlDocument]

Arguments:
  [graphqlDocument]
          

Options:
      --variable <variable>
          Variable in key=value format (coerces booleans, numbers, null; @file reads from path)

      --workspace <slug>
          Target workspace (uses credentials)

      --variables-json <json>
          JSON object of variables (merged with --variable, which takes precedence)

      --paginate
          Auto-paginate a single connection field using cursor pagination

      --silent
          Suppress response output (exit code still reflects errors)

  -h, --help
          Print help (see a summary with '-h')
```
