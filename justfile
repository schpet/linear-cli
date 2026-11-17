# runs the cli from source, e.g. `just dev issue mine`
dev *args:
    cargo run --quiet -- {{ args }}

# installs the cli from this checkout as `linear`
install:
    cargo install --locked --path crates/linear-cli

# checks formatting, runs clippy, and runs the test suite
check:
    cargo fmt --all --check
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace

# refreshes graphql/schema.graphql from linear's api. needs a logged in cli
sync-schema:
    cargo run --quiet -- schema --output graphql/schema.graphql

# writes THIRD_PARTY_LICENSES.md, the dependency license notices that releases ship
licenses:
    cargo xtask licenses

# checks release preparation guards using isolated fixtures
release-check:
    cargo test --locked -p xtask release::

# prepares a stable or alpha version without committing, tagging or pushing
prepare-release version="latest":
    cargo xtask prepare-release {{quote(version)}}

# prepares, checks, commits and publishes exactly one release tag
tag version="latest": (prepare-release version) check
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(svbump read workspace.package.version Cargo.toml)
    jj commit -m "chore: Release linear-cli version $version"
    jj bookmark set main -r @-
    jj git push --bookmark main
    jj tag set "v$version" -r @-
    revision=$(jj log --ignore-working-copy --no-graph -r @- -T commit_id)
    git push origin "$revision:refs/tags/v$version"
    echo "released v$version"

# regenerates .github/workflows/release.yml with the dist version pinned in mise.toml
dist-generate:
    dist generate
    just actions-lock

# needs `gh extension install github/gh-actions-lock`. --no-narrow keeps `v6`
# from being rewritten to `v6.1.0` in the workflow files, which dist would
# reject in release.yml. `just actions-lock --relock` moves tags like `v6` to
# their current commits.
# updates .github/workflows/actions.lock after a workflow's `uses:` lines change
actions-lock *args:
    gh actions-lock --no-narrow {{ args }}
