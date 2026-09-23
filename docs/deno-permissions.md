# Deno Permissions Configuration

This CLI uses `--allow-all` for simplicity. Linear issues can contain images and attachments from arbitrary external domains, making fine-grained `--allow-net` restrictions impractical.

## Files with Permission Flags

| File                    | Purpose                                                 |
| ----------------------- | ------------------------------------------------------- |
| `deno.json`             | `dev`, `install`, `test`, `parity`, `parity:test` tasks |
| `dist-workspace.toml`   | Binary compilation for releases                         |
| `rust/parity/deno.json` | Config and lockfile used by the parity tasks            |

## Why `--allow-all`?

The CLI needs network access to download attachments and images from Linear comments. Since these can be hosted on any domain (e.g., user-uploaded images, external file hosts), maintaining an allow-list is not feasible.

The CLI also requires:

- File system access for config and temp files
- Environment variables for API keys and editor settings
- Subprocess execution for git, editors, and pagers
- System info for hostname

Using `--allow-all` avoids permission errors when Linear content references external resources.

## Parity harness tasks

`deno task parity` runs `rust/parity/runner/main.ts` with `--frozen --allow-all` because the runner spawns `unshare`, `ip`, `setsid`, `jj` and `deno`, stages a module cache, listens on loopback, and manages sandboxes. The runner re-executes itself inside an unprivileged user, network and PID namespace; every program under test receives an explicit environment (synthetic HOME/XDG/APPDATA, staged DENO_DIR, fake key) and cannot reach anything but loopback. `deno task parity:test` runs the harness unit and integration tests under `rust/parity/` with `--frozen --allow-all` for the same subprocess, filesystem and loopback needs. Both tasks use `rust/parity/deno.json`, whose lockfile is derived from the root `deno.lock`, so root test discovery excludes `rust/` and application dependencies stay untouched.
