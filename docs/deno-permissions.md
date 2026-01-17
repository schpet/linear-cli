# Deno Permissions Configuration

This document tracks all locations where Deno permission flags are configured. When adding new permissions (e.g., new network hosts, environment variables), **all files must be updated** to stay in sync.

## Permission Types

| Permission      | Purpose                                                       |
| --------------- | ------------------------------------------------------------- |
| `--allow-env`   | Environment variable access (API keys, config, editor, pager) |
| `--allow-net`   | Network access to Linear API and file storage                 |
| `--allow-read`  | File system read access                                       |
| `--allow-write` | File system write access                                      |
| `--allow-run`   | Execute subprocesses (git, jj, editor, pager)                 |
| `--allow-sys`   | System info access (hostname)                                 |

## Network Hosts

The following hosts must be allowed for full functionality:

- `api.linear.app` - GraphQL API
- `uploads.linear.app` - Private file uploads/downloads
- `public.linear.app` - Public image downloads

## Files to Update

### Primary Configuration

These files define permissions for production use:

| File                  | Lines      | Purpose                                    |
| --------------------- | ---------- | ------------------------------------------ |
| `deno.json`           | 8-9, 14-15 | `dev`, `install`, `test`, `snapshot` tasks |
| `dist-workspace.toml` | 16         | Binary compilation for releases            |

### Test Configuration

Test files define their own `denoArgs` arrays. Most use a shared helper:

| File                                         | Lines | Notes                                           |
| -------------------------------------------- | ----- | ----------------------------------------------- |
| `test/utils/test-helpers.ts`                 | 5-9   | Shared `commonDenoArgs` used by milestone tests |
| `test/commands/issue/issue-view.test.ts`     | 10-14 | Local `denoArgs`                                |
| `test/commands/issue/issue-describe.test.ts` | 7-11  | Local `denoArgs`                                |
| `test/commands/issue/issue-commits.test.ts`  | 6-10  | Local `denoArgs`                                |
| `test/commands/team/team-list.test.ts`       | 8-12  | Local `denoArgs`                                |
| `test/commands/project/project-view.test.ts` | 7-11  | Local `denoArgs`                                |

Note: Test files often use broader permissions (e.g., `--allow-net` without host restrictions) since they run against mock servers.

## Environment Variables

### Runtime Variables

Used by the CLI during normal operation:

```
GITHUB_*, GH_*     - GitHub integration
LINEAR_*           - Linear API key and config
NODE_ENV           - Environment detection
EDITOR             - Text editor for descriptions
PAGER              - Pager for long output
NO_COLOR           - Disable color output
TMPDIR, TMP, TEMP  - Temp directory for downloads
XDG_CONFIG_HOME    - Config file location (Linux)
HOME               - Home directory
APPDATA            - Config file location (Windows)
```

### Test-Only Variables

Additional variables needed for tests:

```
SNAPSHOT_TEST_NAME        - Cliffy snapshot testing
CLIFFY_SNAPSHOT_FAKE_TIME - Time mocking in tests
PATH                      - Process execution
SystemRoot                - Windows compatibility
MOCK_GIT_BRANCH_COMMAND   - Git mocking in tests
TEST_CURRENT_TIME         - Time mocking
```

## Checklist for Adding Permissions

When adding a new permission:

1. [ ] Update `deno.json` tasks: `dev`, `install`, `test`, `snapshot`
2. [ ] Update `dist-workspace.toml` build command
3. [ ] Update `test/utils/test-helpers.ts` if tests need it
4. [ ] Update individual test files if they have local `denoArgs`
5. [ ] Document the new permission in this file
