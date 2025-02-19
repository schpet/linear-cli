dev *args:
    deno run main.ts {{ args }}

# tags the newest release in the changelog
tag:
    deno check main.ts
    deno fmt --check

    svbump write "$(changelog version latest)" version deno.json
    svbump write "$(svbump read version deno.json)" package.version dist-workspace.toml
    deno check

    git commit deno.json dist-workspace.toml CHANGELOG.md -m "chore: Release linear-cli version $(svbump read version deno.json)"
    git tag "v$(svbump read version deno.json)"

    @echo "tagged v$(svbump read version deno.json)"
    @echo
    @echo "run this to release it:"
    @echo
    @echo "  git push origin HEAD --tags"
