import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/template/template-list.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { assertStringIncludes } from "@std/assert"
import {
  captureCommandError,
  commonDenoArgs,
  resolveTeamMock,
} from "../../utils/test-helpers.ts"

const ENG = { id: "team-eng-id", key: "ENG", name: "Engineering" }
const OPS = { id: "team-ops-id", key: "OPS", name: "Operations" }

function template(
  id: string,
  name: string,
  type: string,
  team: { id: string; key: string; name: string } | null,
  extra: { hasFormFields?: boolean; templateData?: string } = {},
) {
  return {
    id,
    name,
    description: `${name} description`,
    type,
    icon: null,
    color: null,
    hasFormFields: extra.hasFormFields ?? false,
    lastAppliedAt: null,
    sortOrder: 0,
    createdAt: "2024-01-01T00:00:00.000Z",
    updatedAt: "2024-01-02T00:00:00.000Z",
    team,
    inheritedFrom: null,
    creator: { id: "user-1", name: "Sam" },
    templateData: extra.templateData ?? '{"title":"Untitled"}',
  }
}

// Deliberately unsorted: the command orders by type, then name, then scope.
const TEMPLATES = [
  template("tpl-kickoff", "Kickoff", "project", null, {
    templateData: '{"name":"Kickoff: ","priority":3}',
  }),
  template("tpl-bug-ops", "Bug report", "issue", OPS),
  template("tpl-intake-eng", "Intake form", "issue", ENG, {
    hasFormFields: true,
  }),
  template("tpl-design", "Design doc", "document", ENG),
  template("tpl-bug-eng", "Bug report", "issue", ENG, {
    templateData: '{"title":"Bug: ","priority":2,"labelIds":["label-bug"]}',
  }),
  // Same type and name as the team ones: the workspace template sorts first.
  template("tpl-bug-workspace", "Bug report", "issue", null),
]

function templatesMock(templates: unknown[]) {
  return {
    queryName: "GetTemplates",
    response: { data: { templates } },
  }
}

async function withServer(
  mocks: ConstructorParameters<typeof MockLinearServer>[0],
  run: () => Promise<unknown>,
) {
  const server = new MockLinearServer(mocks)
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await run()
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

await snapshotTest({
  name: "Template List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

await snapshotTest({
  name: "Template List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([templatesMock(TEMPLATES)], () => listCommand.parse())
  },
})

await snapshotTest({
  name: "Template List Command - Filter By Type",
  meta: import.meta,
  colors: false,
  args: ["--type", "issue"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([templatesMock(TEMPLATES)], () => listCommand.parse())
  },
})

await snapshotTest({
  name: "Template List Command - Filter By Team Keeps Workspace Templates",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer(
      [resolveTeamMock("ENG", ENG), templatesMock(TEMPLATES)],
      () => listCommand.parse(),
    )
  },
})

await snapshotTest({
  name: "Template List Command - Truncates Wide Names By Display Width",
  meta: import.meta,
  colors: false,
  args: ["--type", "document"],
  denoArgs: commonDenoArgs,
  async fn() {
    // 60 CJK characters are 120 terminal columns, wider than the NAME column
    // in a 120-column table; truncation must count columns, not code units.
    await withServer([
      templatesMock([
        template("tpl-wide", "模板".repeat(30), "document", null),
        template("tpl-narrow", "Short", "document", null),
      ]),
    ], () => listCommand.parse())
  },
})

await snapshotTest({
  name: "Template List Command - Empty",
  meta: import.meta,
  colors: false,
  // The only document template belongs to ENG, so OPS sees none.
  args: ["--type", "document", "--team", "OPS"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer(
      [resolveTeamMock("OPS", OPS), templatesMock(TEMPLATES)],
      () => listCommand.parse(),
    )
  },
})

await snapshotTest({
  name: "Template List Command - Empty JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([templatesMock([])], () => listCommand.parse())
  },
})

Deno.test("Template List Command - rejects an unknown --type before any request", async () => {
  const output = await captureCommandError(() =>
    listCommand.parse(["--type", "bogus"])
  )
  assertStringIncludes(output, 'Option "--type"')
  assertStringIncludes(output, "bogus")
})
