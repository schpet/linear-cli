# Contributing

## Scope of the issue tracker

Issues are for bugs and feature requests in linear-cli, filed by people using it. Questions about installing, configuring, or using it belong in [Discussions](https://github.com/schpet/linear-cli/discussions).

Do not open issues recommending that linear-cli adopt a library you maintain or are affiliated with. These are closed without discussion.

If you hit a problem using linear-cli that a dependency would solve, file the problem. Naming a library as part of that is fine, but the issue needs to be about the problem you hit, not the library.

Issues outside this scope may be closed.

## Development

linear-cli is written in TypeScript and runs on [Deno](https://deno.com).

```sh
deno task test      # run the test suite
deno task validate  # type check, format, lint
deno task snapshot  # update snapshot tests
```

After editing a GraphQL document, run `deno task codegen` to regenerate types.

## Pull requests

Keep changes focused, and add tests for new behavior. Tests mirror the source layout: `src/commands/issue/issue-view.ts` is tested by `test/commands/issue/issue-view.test.ts`.
