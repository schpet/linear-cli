dev *args:
    deno run main.ts {{ args }}

# release a major, minor or patch version
release level:
    svbump write {{ level }} version deno.json
    svbump write "$(svbump read version deno.json)" package.version dist-workspace.toml
    git commit deno.json dist-workspace.toml -m "chore: Release linear-cli version $(svbump read version deno.json)"
    git tag "v$(svbump read version deno.json)"

    @echo "tagged v$(svbump read version deno.json)"
    @echo
    @echo "run this to release it:"
    @echo
    @echo "  git push origin HEAD --tags"

release-preview level:
  svbump preview {{ level }} version deno.json
