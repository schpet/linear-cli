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

# regenerates skills/linear-cli/SKILL.md and its references from the cli's help
skill-docs:
    cargo run --locked --quiet --example skill_docs

# refreshes graphql/schema.graphql from linear's api. needs a logged in cli
sync-schema:
    cargo run --quiet -- schema --output graphql/schema.graphql

# writes THIRD_PARTY_LICENSES.md, the dependency license notices that releases ship
licenses:
    cargo xtask licenses

# sets the claude code plugin versions to the version in Cargo.toml
plugin-version:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(svbump read workspace.package.version Cargo.toml)"
    for file in .claude-plugin/plugin.json .claude-plugin/marketplace.json; do
        jq --arg v "$version" '.version = $v | if .plugins then .plugins[0].version = $v else . end' "$file" > "$file.tmp"
        mv "$file.tmp" "$file"
    done

# tags the newest release in the changelog
tag: check
    svbump write "$(changelog version latest)" workspace.package.version Cargo.toml
    cargo update --workspace
    just skill-docs
    just plugin-version

    jj commit -m "chore: Release linear-cli version $(svbump read workspace.package.version Cargo.toml)"
    jj bookmark set main -r @-
    jj tag set "v$(svbump read workspace.package.version Cargo.toml)" -r @-
    jj git push --bookmark main

    git push origin --tags

    @echo "released v$(svbump read workspace.package.version Cargo.toml)"

# regenerates .github/workflows/release.yml with the dist version pinned in mise.toml
dist-generate:
    dist generate

claude-remove-local:
    -claude plugin remove linear-cli@linear-cli
    -claude plugin marketplace remove linear-cli

claude-install-local:
    claude plugin marketplace add ./
    claude plugin install linear-cli@linear-cli

claude-install-github:
    claude plugin marketplace add schpet/linear-cli
    claude plugin install linear-cli@linear-cli
