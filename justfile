dev *args:
    deno run -A main.ts {{ args }}

# tags the newest release in the changelog
tag:
    deno check main.ts
    deno fmt --check
    deno lint

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
