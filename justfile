dev *args:
    deno run -A src/main.ts {{ args }}

# tags the newest release in the changelog
tag:
    deno check src/main.ts
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

# regenerates .github/workflows/release.yml. needs the `dist` version pinned by
# cargo-dist-version in dist-workspace.toml, or it rewrites that pin to whatever
# version you ran: mise use -g "ubi:axodotdev/cargo-dist[exe=dist]@0.31.0"
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
