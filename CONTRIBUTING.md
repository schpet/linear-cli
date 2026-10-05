# Contributing

## Scope of the issue tracker

Issues are for bugs and feature requests in linear-cli, filed by people using it. Questions about installing, configuring, or using it belong in [Discussions](https://github.com/schpet/linear-cli/discussions).

Do not open issues recommending that linear-cli adopt a library you maintain or are affiliated with. These are closed without discussion.

If you hit a problem using linear-cli that a dependency would solve, file the problem. Naming a library as part of that is fine, but the issue needs to be about the problem you hit, not the library.

Issues outside this scope may be closed.

## Development

linear-cli is written in Rust. The toolchain version is pinned in `rust-toolchain.toml`, which rustup picks up automatically.

```sh
cargo run -- issue list   # run the CLI from source
cargo test --workspace    # run the test suite
just check                # format check, clippy, and tests, as CI runs them
```

After changing commands, flags, or help text, run `just skill-docs` to regenerate the agent skill documentation.

## Pull requests

Keep changes focused, and add tests for new behavior. Command behavior is tested in `crates/linear-cli/tests/cli/`, which runs the `linear` binary against a mock Linear API, with one module per command group.
