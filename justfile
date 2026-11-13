# runs the cli from source, e.g. `just dev issue list`
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

# tags the newest release in the changelog
tag: check
    svbump write "$(changelog version latest)" workspace.package.version Cargo.toml
    cargo update --workspace

    jj commit -m "chore: Release linear-cli version $(svbump read workspace.package.version Cargo.toml)"
    jj bookmark set main -r @-
    jj tag set "v$(svbump read workspace.package.version Cargo.toml)" -r @-
    jj git push --bookmark main

    git push origin --tags

    @echo "released v$(svbump read workspace.package.version Cargo.toml)"

# regenerates .github/workflows/release.yml with the dist version pinned in mise.toml
dist-generate:
    dist generate
