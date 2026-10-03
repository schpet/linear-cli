---
name: Release
description: This skill should be used when the user asks to "make a release", "create a release", "cut a release", "release a new version", "publish a release", or mentions preparing for release. Provides comprehensive workflow for reviewing changes, updating changelog, determining semver bump, and publishing.
version: 0.1.0
---

# Release Workflow

This skill provides a systematic workflow for creating and publishing releases for the linear-cli project. It handles changelog management, version bumping, testing, and tagging.

## When to Use

Use this skill when preparing to release a new version of linear-cli. The workflow ensures all changes are documented, tests pass, and versions are properly tagged before publishing.

## Prerequisites

Ensure the following tools are available:

- `changelog` skill for changelog management
- `svbump` for version bumping (installed)
- `jj` for version control operations
- `just` for running the release tasks
- `jq` for updating the Claude Code plugin versions

## Release Workflow

### Step 1: Review Commits Since Last Release

Determine the commits that have been made since the last release:

```bash
jj log --ignore-working-copy --git -r 'tags()..@' --no-graph
```

This shows all commits from the most recent tag to the current commit.

### Step 2: Add Changelog Entries

For each commit identified above, evaluate whether it warrants a changelog entry. Focus on user-facing changes:

**Include in changelog:**

- New features
- Bug fixes
- Breaking changes
- Significant improvements
- Deprecations

**Exclude from changelog:**

- Internal refactoring without user impact
- Documentation-only changes
- Build/CI configuration changes
- Chore commits (unless significant)

Use the changelog CLI to add entries. Use `--attribute-pr` with the commit SHA to automatically look up the associated PR and add attribution, excluding `schpet` and `schpetbot`:

```bash
changelog add --type <type> "<description>" --attribute-pr <commit-sha> --exclude-users schpet,schpetbot
```

Omit `--attribute-pr` for commits without an associated PR or when attribution isn't relevant.

Types match Keep a Changelog categories:

- `added` - New features
- `changed` - Changes in existing functionality
- `deprecated` - Soon-to-be removed features
- `removed` - Removed features
- `fixed` - Bug fixes
- `security` - Security improvements

### Step 3: Verify Changelog with User

After adding all relevant changelog entries, show the unreleased section of CHANGELOG.md to the user and ask them to review it:

1. Read the CHANGELOG.md file
2. Show the `[Unreleased]` section
3. Ask: "Please review these changelog entries. Are there any changes needed before release?"
4. Make any requested adjustments

### Step 4: Determine Semver Bump

Based on the types of changes in the changelog, determine and recommend the appropriate semantic version bump:

**Major (X.0.0):**

- Breaking changes
- Removed features
- Significant API changes

**Minor (0.X.0):**

- New features (added)
- Deprecations
- Backward-compatible functionality additions

**Patch (0.0.X):**

- Bug fixes
- Security fixes
- Minor improvements with no new features

Present the recommendation to the user:

```
Based on the changelog entries, I recommend a <MAJOR/MINOR/PATCH> version bump because:
- [reason 1]
- [reason 2]

Current version: <current>
Proposed version: <proposed>

Should I proceed with this version bump?
```

Wait for user confirmation before proceeding.

### Step 5: Run Changelog Release

Once the user confirms the version bump, run the changelog release command with the appropriate semver level:

```bash
changelog release <major|minor|patch>
```

This updates CHANGELOG.md, converting the Unreleased section to a versioned release.

### Step 6: Execute Tag Process

After the changelog is released, run the tag recipe:

```bash
just tag
```

It does the whole release in order, stopping at the first failure:

1. **Quality checks** (`just check`): `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace`
2. **Version bump:** writes the latest changelog version to `workspace.package.version` in `Cargo.toml` with `svbump`, then refreshes `Cargo.lock` with `cargo update --workspace`
3. **Skill docs** (`just skill-docs`): regenerates `skills/linear-cli/SKILL.md` and its references from the CLI's help
4. **Plugin versions** (`just plugin-version`): sets `version` in `.claude-plugin/plugin.json` and both `version` and `plugins[0].version` in `.claude-plugin/marketplace.json`
5. **Commit and tag:** `jj commit -m "chore: Release linear-cli version <version>"`, then moves the `main` bookmark and sets the `v<version>` tag on that commit (`@-`)
6. **Push:** `jj git push --bookmark main`, then `git push origin --tags`

When it finishes it prints `released v<version>`.

## Error Handling

If any step fails:

- **Quality checks fail:** Fix the issues before continuing. Do not proceed with release if tests fail or linting errors exist.
- **Version bump fails:** Verify the version format and files exist.
- **Push fails:** Check authentication and remote access.

Always stop and report errors clearly. Never continue the release process if a critical step fails.

## Important Notes

- The justfile `tag` recipe runs every step above; there is no need to run them by hand
- Use `jj` for all version control operations (per the project AGENTS.md)
- Always use `--ignore-working-copy` for read-only jj operations
- The workflow creates a commit on the parent (@-) and then creates a new working commit
- Both `jj git push` and `git push origin --tags` are needed (jj for bookmark, git for tags)

## Post-Release

After successful release:

1. Verify the tag appears on GitHub
2. Check that the GitHub Actions release workflow (`.github/workflows/release.yml`) builds the binaries and publishes the Homebrew formula and npm package
3. Confirm the new version is published

## Reference

See the `tag` and `plugin-version` recipes in the `justfile` for the implementation.
