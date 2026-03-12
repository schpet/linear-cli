---
name: release
description: Create and publish linear-cli releases with changelog management, semver version bumping, and git tagging. Use when the user says "make a release", "cut a release", "publish a new version", "release linear-cli", "bump version", or "tag a release". Handles reviewing commits, adding changelog entries, running quality checks, updating version files, and pushing tags.
---

# Release Workflow

Use `jj` for all version control operations. Use `--ignore-working-copy` for read-only jj operations. Both `jj git push` (bookmark) and `git push origin --tags` (tags) are needed for pushing.

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

After the changelog is released, execute the complete tag process from the justfile. This includes:

1. **Run quality checks:**
   ```bash
   deno check src/main.ts
   deno fmt --check
   deno lint
   deno task test
   ```

2. **Update version files:**
   ```bash
   # Get the latest version from changelog
   LATEST_VERSION=$(changelog version latest)

   # Write version to deno.json
   svbump write "$LATEST_VERSION" version deno.json

   # Read version from deno.json and write to dist-workspace.toml
   DENO_VERSION=$(svbump read version deno.json)
   svbump write "$DENO_VERSION" package.version dist-workspace.toml
   ```

3. **Regenerate skill documentation:**
   ```bash
   # Generate updated skill docs (includes version from deno.json)
   deno task generate-skill-docs
   ```

4. **Create commit and tag:**
   ```bash
   # Get the final version
   FINAL_VERSION=$(svbump read version deno.json)

   # Create commit
   jj commit -m "chore: Release linear-cli version $FINAL_VERSION"

   # Set main bookmark to parent commit
   jj bookmark set main -r @-

   # Create tag on the parent commit
   jj tag set "v$FINAL_VERSION" -r @-
   ```

5. **Push to remote:**
   ```bash
   # Push the bookmark
   jj git push --bookmark main

   # Push tags (using git)
   git push origin --tags
   ```

6. **Report completion:**
   ```
   Released v$FINAL_VERSION successfully!
   ```

## Error Handling

If any step fails:

- **Quality checks fail:** Fix the issues before continuing. Do not proceed with release if tests fail or linting errors exist.
- **Version bump fails:** Verify the version format and files exist.
- **Push fails:** Check authentication and remote access.

Always stop and report errors clearly. Never continue the release process if a critical step fails.

## Reference

See `justfile` lines 5-21 for the complete tag recipe implementation.
