import { fromFileUrl, join } from "@std/path"
import { exportRuntime, type Manifest, readBaseline } from "./verify.ts"
import { withSourceMap } from "./source-map.ts"

const parity = fromFileUrl(new URL("./", import.meta.url))
const baseline = readBaseline(
  JSON.parse(await Deno.readTextFile(join(parity, "baseline.json"))),
)
const manifest: Manifest = {
  baseline,
  routes: (await withSourceMap(await exportRuntime())).map((route) => ({
    ...route,
    fixtureStatus: "pending",
    qaStatus: "pending",
    liveStatus: "pending",
  })),
  probes: [
    { args: ["--version"], exitCode: 0, stdoutIncludes: ["linear 2.6.0"] },
    {
      args: ["--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear",
        "Handy linear commands",
        "completions",
      ],
    },
    {
      args: ["issue", "mine", "--help"],
      exitCode: 0,
      stdoutIncludes: ["Usage:   linear issue mine", "List your issues"],
    },
    {
      args: ["issue", "query", "--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear issue query",
        "Query issues with structured filters",
        "--json",
      ],
    },
    {
      args: ["document", "--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear document",
        "Manage Linear documents",
        "List documents",
      ],
    },
    {
      args: ["label", "list", "--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear label list",
        "List issue labels",
        "Show only workspace-level labels",
      ],
    },
    {
      args: ["completions", "--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear completions",
        "Generate shell completions",
        "bash",
        "fish",
        "zsh",
      ],
    },
    {
      args: ["completions", "bash", "--help"],
      exitCode: 0,
      stdoutIncludes: [
        "Usage:   linear completions bash",
        "Generate shell completions for bash",
      ],
    },
  ],
  notes: {
    helpVersion:
      'Cliffy lazily registers -h, --help ("Show this help.") as a global option first in every route option list; it registers -V, --version ("Show the version number for this program.") only at root, second there before --workspace. These options are omitted from the pre-registration inventory and corroborated by runner/cases/help-root.json.',
    parentActions:
      "pending_safe_fixture; public Cliffy inspection does not expose action handler",
    workspaceCollision:
      "pending_safe_fixture; root global and label list local definitions both exported",
    valueHandlers:
      "unknown_function when registered; runtime function body is deliberately not serialized",
    sourceMapping:
      "exact source files and C work-item IDs derived from rust/WORK_ITEMS.md; shared files are marked",
    routeSource:
      "Cliffy public command, option, type, example, environment, usage, argument, alias, and child getters",
  },
}
await Deno.writeTextFile(
  join(parity, "manifest.json"),
  JSON.stringify(manifest, null, 2) + "\n",
)
console.log(`wrote ${manifest.routes.length} routes`)
