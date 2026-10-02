# R02LEX shared lexical path normalization

Status: completed as one local R02LEX commit above integrated R02B3 `4bc0b6cb`, awaiting coordinator adoption. The ignored `breaking-v3/R02LEX-plan.md` received independent Claude plan SHIP after a Windows prefix clarification. A fresh read-only Claude whole-diff review returned **SHIP**, with no correctness blocker. Its three doc suggestions are incorporated below and in the compatibility/state files.

The shared `config::source::lexical` helper now pops only a preceding normal component for `..`, preserves unresolved relative parents, and clamps at a root. Public config and credential path tests exercise `/..` and `../../`; the credential test also exercises `a/../../b`. The change affects config candidates from cwd/Git, dotenv candidates and credential paths through the same helper. It makes no file read or network request and adds no dependency/lock change. Native Windows drive-relative, UNC and verbatim paths remain for G02 qualification; Unix tests with `OsFamily::Windows` cannot verify native Windows `PathBuf` semantics.

Permission-free pure Deno oracle: `deno eval 'import { join } from "@std/path"; for (const p of ["/..", "../../", "a/../../b", "/", "./", "a//b", "a/../b"]) console.log(JSON.stringify({input:p, config:join(p,"linear","linear.toml"),credentials:join(p,"linear","credentials.toml")}))'`. Deno 2.7.9 resolved lock-pinned `@std/path` 1.1.4. Exact results:

| Base        | Config path                | Credential path                 |
| ----------- | -------------------------- | ------------------------------- |
| `/..`       | `/linear/linear.toml`      | `/linear/credentials.toml`      |
| `../../`    | `../../linear/linear.toml` | `../../linear/credentials.toml` |
| `a/../../b` | `../b/linear/linear.toml`  | `../b/linear/credentials.toml`  |
| `/`         | `/linear/linear.toml`      | `/linear/credentials.toml`      |
| `./`        | `linear/linear.toml`       | `linear/credentials.toml`       |
| `a//b`      | `a/b/linear/linear.toml`   | `a/b/linear/credentials.toml`   |
| `a/../b`    | `b/linear/linear.toml`     | `b/linear/credentials.toml`     |

`deno.lock` SHA-256 `3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403`; frozen `src/config.ts` `f3c3897e3010f4d659a38686537897e3fcbca8f582dbd5616d65db94fb1a85e2`; frozen `src/credentials.ts` `8bc78f4390366f37a064ca4cc882f036e009eb7115e0224d622b9dcd8f0f92b3`.

Verification: pinned Rust 1.93.0 `cargo fmt --all --check`, locked/offline all-target workspace `cargo check`, all-target `cargo clippy -- -D warnings` (18m19s), focused public auth path 2/2 and config discovery 1/1, and full all-target workspace tests **269/269** passed. The `qa-review` case table at `~/.local/state/qa/linear-cli/qa-r02lex.md` records 4/5 passed, zero failures and one native Windows case blocked until G02. The fresh Claude whole-diff review returned SHIP; it independently checked all six tracked files, the helper's callers, hashes and Deno table, without running code. No live Linear operation applies to this path-only foundation; no host credential or keyring was read. The local jj commit leaves a fresh empty scratch `@`; neither `rust-port` nor main is moved here, and nothing is pushed.
