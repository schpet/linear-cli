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

    jj commit -m "chore: Release linear-cli version $(svbump read version deno.json)"
    jj bookmark set main -r @-
    jj tag set "v$(svbump read version deno.json)" -r @-
    jj git push --bookmark main

    git push origin --tags

    @echo "released v$(svbump read version deno.json)"

# depends on `cargo install --git https://github.com/astral-sh/cargo-dist.git --tag v0.28.3 cargo-dist`
# cargo-dist - needed to update .github/workflows/release.yml
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
