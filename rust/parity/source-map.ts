import { fromFileUrl, join } from "@std/path"

const root = fromFileUrl(new URL("../../", import.meta.url))

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function groupSource(path: string): string {
  const parts = path.split(" ").slice(1)
  if (parts.length === 0) return "src/cli.ts"
  if (parts.length === 1) return `src/commands/${parts[0]}/${parts[0]}.ts`
  return `src/commands/${parts[0]}/${parts[0]}-${parts.slice(1).join("-")}.ts`
}

export async function withSourceMap(
  routes: Array<Record<string, unknown>>,
): Promise<Array<Record<string, unknown>>> {
  const text = await Deno.readTextFile(join(root, "rust/WORK_ITEMS.md"))
  const items = new Map<string, { id: string; source: string }>()
  for (const line of text.split("\n")) {
    const columns = line.split("|").map((part) => part.trim())
    if (!/^C\d{3}$/.test(columns[1] ?? "")) continue
    const id = columns[1]
    const command = columns[2]?.match(/^`([^`]+)`$/)?.[1]
    const source = columns[4]?.match(/^`(src\/[^`]+)`$/)?.[1]
    assert(command != null && source != null, `invalid work item ${id}`)
    assert(!items.has(command), `duplicate work item for ${command}`)
    items.set(command, { id, source: source.split(" (")[0] })
  }
  assert(items.size === 86, `expected 86 command work items; got ${items.size}`)
  const seen = new Set<string>()
  const enriched = routes.map((route) => {
    assert(
      typeof route.path === "string" && typeof route.kind === "string",
      "invalid route",
    )
    const path = route.path.slice("linear".length).trim()
    let source: string
    let workItem: string
    let sourceKind: string
    if (route.kind === "source_leaf") {
      const item = items.get(path)
      assert(item != null, `no work item for ${route.path}`)
      source = item.source
      workItem = item.id
      sourceKind = path === "completions"
        ? "cliffy_registration"
        : "command_implementation"
      seen.add(path)
    } else if (route.kind === "generated_completion_child") {
      source = "src/cli.ts"
      workItem = "C086"
      sourceKind = "cliffy_generated"
    } else {
      source = groupSource(route.path)
      workItem = "F01"
      sourceKind = "group_registration"
    }
    return { ...route, source, workItem, sourceKind }
  })
  assert(
    seen.size === items.size,
    `unmatched work items: ${
      [...items.keys()].filter((key) => !seen.has(key)).join(", ")
    }`,
  )
  const useCount = new Map<string, number>()
  for (const route of enriched) {
    assert(typeof route.source === "string", "route source missing")
    useCount.set(route.source, (useCount.get(route.source) ?? 0) + 1)
  }
  for (const route of enriched) {
    assert(typeof route.source === "string", "route source missing")
    await Deno.stat(join(root, route.source))
  }
  return enriched.map((route) => ({
    ...route,
    sourceKind: route.sourceKind === "command_implementation" &&
        useCount.get(route.source) !== 1
      ? "shared_command_implementation"
      : route.sourceKind,
  }))
}
