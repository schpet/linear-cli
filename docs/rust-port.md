# Native Rust local candidate

The major-3 candidate is `3.0.0-alpha.1`. This is a local reviewed port; the
existing Deno/JSR installer and published releases remain available. Nothing in
this development workflow publishes an artifact or replaces an installed CLI.

Build from the repository root with `cd rust` followed by
`cargo +1.93.0 build --locked --release --bin linear`. The authoritative schema
is `graphql/schema.graphql`; keep it alongside the `rust/` workspace when using
a source archive. The binary runs without Deno, Node or npm. Deno remains a
development dependency for the frozen reference and parity tests.

Native clap owns flags, errors, help and completions. Strict typed inputs and
responses have documented native boundaries; read the compatibility ledger
instead of assuming Cliffy parsing or every platform interaction is identical.
Successful JSON preserves GraphQL fields/connections and script output.

Run the candidate by its absolute path. For a private local install, extract the
verified native host archive to a temporary prefix and check its recorded SHA.
The repository's installed CLI, user configuration and credential stores need
not change. Shell/npm/Homebrew/updater rehearsal evidence is recorded separately
from host binary installation and from unbuilt foreign targets.

Generate bash, zsh or fish scripts with `linear completions <shell>` and use the
instructions in `linear completions --help`. Generate native skill references
from the explicit SHA-pinned binary and the development typed-clap manifest;
never depend on whichever `linear` happens to be on PATH. The original Deno
documentation task remains available for the source reference.

Native source archives contain the Rust workspace, build inputs, root license
and authoritative schema. Binary archives include root, GraphQL-JS and
dependency notices in distinct paths. A license-file availability
inventory is packaging provenance, not a claim that every target was built.

Acceptance records distinguish code completeness and available-box checks from
unperformed native OS/service/TTY/live cases. No fully verified platform claim
is made solely from cross-checking or fake backend tests. Publication, tags,
default-installer cutover and user installations require a separate later action.
