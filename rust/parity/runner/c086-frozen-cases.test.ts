import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { decodeByteValue, sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c086-frozen-cases/", import.meta.url).pathname
const ids = [
  "c086-bash-help",
  "c086-bash-hyphen-name",
  "c086-bash-malformed-credential",
  "c086-bash-metachar-name",
  "c086-bash-short-name",
  "c086-bash-startup-warning",
  "c086-bash-stdout-after-bytes",
  "c086-bash-stdout-closed",
  "c086-bash",
  "c086-complete-agent-session-alias",
  "c086-complete-agent-session-status",
  "c086-complete-boolean-nested",
  "c086-complete-boolean-root",
  "c086-complete-help",
  "c086-complete-hidden-path",
  "c086-complete-literal-boundary",
  "c086-complete-malformed-credential",
  "c086-complete-no-action",
  "c086-complete-number-milestone",
  "c086-complete-number",
  "c086-complete-partial-command",
  "c086-complete-root-workspace",
  "c086-complete-sort-alias-path",
  "c086-complete-sort-issue-mine",
  "c086-complete-sort-parent",
  "c086-complete-sort-short-alias",
  "c086-complete-stdout-closed",
  "c086-complete-string",
  "c086-complete-template-type",
  "c086-complete-unknown-action",
  "c086-complete-unknown-command",
  "c086-complete-unknown-no-suggestion",
  "c086-complete-variable-api",
  "c086-complete-workspace-rejected",
  "c086-fish-help",
  "c086-fish-long-name",
  "c086-fish",
  "c086-name-empty",
  "c086-name-missing",
  "c086-name-repeated",
  "c086-parent-bare",
  "c086-parent-help-color",
  "c086-parent-help",
  "c086-root-workspace-bash",
  "c086-shell-extra-argument",
  "c086-shell-workspace-rejected",
  "c086-unknown-shell",
  "c086-zsh-equals-name",
  "c086-zsh-help",
  "c086-zsh",
]
const fixtureFiles = [
  "fixtures/c086-invalid-default/linear/credentials.toml",
  "fixtures/c086-malformed/linear/credentials.toml",
]
const routes = new Set([
  "linear completions",
  "linear completions bash",
  "linear completions fish",
  "linear completions zsh",
  "linear completions complete",
])
// Sorted "relative path NUL file SHA-256 LF" lines for every case and fixture.
const bundleSha256 =
  "a577683ab1b041f822e3385abc5bf9df46252c5112b9a0d20d348b3925b296bf"
// The frozen default scripts equal the C086 reconnaissance captures.
const defaultScripts: Record<string, [number, string]> = {
  "c086-bash": [
    56526,
    "ed4766875f2605efdc85af978ad6e2bd032a1d8e1e6f858cf2b1c603fd3105f4",
  ],
  "c086-fish": [
    120118,
    "b67237c5a64aa58554923d652572e7fc3ac309f43b3863739b8719a59dc22183",
  ],
  "c086-zsh": [
    110440,
    "1436e1ec611f2cd2f02799f43423fd480a0880c35724f7463426e07a3d623e0d",
  ],
}
// Hidden complete output is joined with LF and has no trailing newline.
const completeValues: Record<string, string> = {
  "c086-complete-sort-issue-mine": "manual\npriority",
  "c086-complete-sort-alias-path": "manual\npriority",
  "c086-complete-sort-short-alias": "manual\npriority",
  "c086-complete-boolean-root": "true\nfalse",
  "c086-complete-boolean-nested": "true\nfalse",
  "c086-complete-agent-session-status":
    "pending\nactive\ncomplete\nawaitingInput\nerror\nstale",
  "c086-complete-agent-session-alias":
    "pending\nactive\ncomplete\nawaitingInput\nerror\nstale",
  "c086-complete-template-type": "issue\nproject\ndocument",
  "c086-complete-sort-parent": "",
  "c086-complete-string": "",
  "c086-complete-number": "",
  "c086-complete-number-milestone": "",
  "c086-complete-variable-api": "",
  "c086-complete-unknown-action": "",
  "c086-complete-literal-boundary": "",
  "c086-complete-root-workspace": "manual\npriority",
}

async function filesUnder(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(directory)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory && !entry.isSymlink) {
      files.push(...await filesUnder(join(directory, entry.name), relative))
    } else if (entry.isFile && !entry.isSymlink) {
      files.push(relative)
    } else {
      throw new Error(`unexpected C086 entry ${relative}`)
    }
  }
  return files
}

Deno.test("C086 freezes the exact completions corpus without claiming Rust parity", async () => {
  const files = (await filesUnder(root)).sort()
  assertEquals(
    files,
    [...ids.map((id) => `${id}.json`), ...fixtureFiles].sort(),
  )
  const lines: string[] = []
  for (const file of files) {
    lines.push(
      `${file}\0${await sha256Hex(await Deno.readFile(join(root, file)))}\n`,
    )
  }
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
  )

  const loaded = await loadCases(root, routes, "c086-")
  assertEquals(loaded.map((entry) => entry.spec.id), ids)
  assertEquals(
    new Set(loaded.map((entry) => entry.spec.route)),
    routes,
  )
  let scripts = 0
  let completes = 0
  for (const { spec } of loaded) {
    assertEquals(spec.cwdFixture, "empty", spec.id)
    assertEquals(spec.env.PATH, "{{bin}}", spec.id)
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1", spec.id)
    assertEquals(
      spec.env.LINEAR_GRAPHQL_ENDPOINT,
      "http://127.0.0.1:1/graphql",
      spec.id,
    )
    assertEquals(spec.env.LINEAR_API_KEY, undefined, spec.id)
    assertEquals(
      spec.env.NO_COLOR,
      spec.id === "c086-parent-help-color" ? undefined : "1",
      spec.id,
    )
    assertEquals(spec.fixtureServer, null, spec.id)
    assertEquals(spec.graphql, undefined, spec.id)
    assertEquals(spec.gitProbe, undefined, spec.id)
    assertEquals(spec.deviation, null, spec.id)
    assertEquals(spec.expected.fileEffects, [], spec.id)
    assert(
      spec.configFixture == null ||
        ["c086-invalid-default", "c086-malformed"].includes(
          spec.configFixture,
        ),
      spec.id,
    )
    const stdout = spec.expected.stdout
    if ("mode" in stdout) continue
    const text = new TextDecoder().decode(decodeByteValue(stdout))
    const pinned = defaultScripts[spec.id]
    if (pinned != null) {
      scripts += 1
      assertEquals(new TextEncoder().encode(text).length, pinned[0], spec.id)
      assertEquals(
        await sha256Hex(new TextEncoder().encode(text)),
        pinned[1],
        spec.id,
      )
    }
    const values = completeValues[spec.id]
    if (values != null) {
      completes += 1
      assertEquals(spec.route, "linear completions complete", spec.id)
      assertEquals(spec.expected.exit, { code: 0 }, spec.id)
      assertEquals(text, values, spec.id)
    }
  }
  assertEquals(scripts, 3)
  assertEquals(completes, Object.keys(completeValues).length)
})
