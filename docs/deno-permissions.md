# Deno Permissions Configuration

This CLI uses `--allow-all` for simplicity. Linear issues can contain images and attachments from arbitrary external domains, making fine-grained `--allow-net` restrictions impractical.

## Files with Permission Flags

| File                  | Purpose                         |
| --------------------- | ------------------------------- |
| `deno.json`           | `dev`, `install`, `test` tasks  |
| `dist-workspace.toml` | Binary compilation for releases |

## Why `--allow-all`?

The CLI needs network access to download attachments and images from Linear comments. Since these can be hosted on any domain (e.g., user-uploaded images, external file hosts), maintaining an allow-list is not feasible.

The CLI also requires:

- File system access for config and temp files
- Environment variables for API keys and editor settings
- Subprocess execution for git, editors, and pagers
- System info for hostname

Using `--allow-all` avoids permission errors when Linear content references external resources.
