dev *args:
    deno run -A src/main.ts {{ args }}

# tags the newest release in the changelog
tag:
    deno check --all
    deno fmt --check
    deno lint
    deno task test

    svbump write "$(changelog version latest)" version deno.json
    svbump write "$(svbump read version deno.json)" package.version dist-workspace.toml

    git add deno.json dist-workspace.toml CHANGELOG.md
    git commit -m "chore: Release linear-cli version $(svbump read version deno.json)"
    git tag "v$(svbump read version deno.json)"

    @echo "tagged v$(svbump read version deno.json)"
    @echo
    @echo "run this to release it:"
    @echo
    @echo "  git push origin HEAD --tags"

# depends on `cargo install --git https://github.com/astral-sh/cargo-dist.git --tag v0.28.3 cargo-dist`
# cargo-dist - needed to update .github/workflows/release.yml
dist-generate:
  dist generate
