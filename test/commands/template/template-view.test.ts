import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/template/template-view.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { assertStringIncludes } from "@std/assert"
import {
  captureCommandError,
  commonDenoArgs,
} from "../../utils/test-helpers.ts"

const ENG = { id: "team-eng-id", key: "ENG", name: "Engineering" }
const BUG_ID = "11111111-1111-4111-8111-111111111111"

// The body shape Linear stores for an issue template: ProseMirror JSON under
// descriptionData, never markdown.
const BUG_TEMPLATE_DATA = JSON.stringify({
  title: "Bug: ",
  priority: 2,
  estimate: 3,
  labelIds: ["label-bug"],
  stateId: "state-todo",
  subIssueData: [
    {
      title: "Write regression test",
      description: "Cover the bug with a test",
      labelIds: ["label-test"],
    },
    { title: "Fix it" },
  ],
  descriptionData: {
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 2 },
        content: [{ type: "text", text: "Steps to reproduce" }],
      },
      {
        type: "ordered_list",
        attrs: { order: 1 },
        content: [{ type: "list_item", content: [{ type: "paragraph" }] }],
      },
      {
        type: "heading",
        attrs: { level: 2 },
        content: [{ type: "text", text: "Expected" }],
      },
      {
        type: "paragraph",
        content: [
          { type: "text", text: "Describe the " },
          { type: "text", text: "expected", marks: [{ type: "bold" }] },
          { type: "text", text: " behaviour." },
        ],
      },
    ],
  },
})

const BUG_TEMPLATE = {
  id: BUG_ID,
  name: "Bug report",
  description: "Standard bug intake",
  type: "issue",
  icon: null,
  color: null,
  hasFormFields: false,
  lastAppliedAt: null,
  sortOrder: 0,
  // Noon UTC: formatRelativeTime falls back to toLocaleDateString for old
  // dates, and a midnight timestamp lands on a different calendar day in CI
  // (UTC) than on a Pacific-time machine.
  createdAt: "2024-01-01T12:00:00.000Z",
  updatedAt: "2024-01-02T12:00:00.000Z",
  team: ENG,
  inheritedFrom: null,
  creator: { id: "user-1", name: "Sam" },
  templateData: BUG_TEMPLATE_DATA,
}

const KICKOFF_TEMPLATE = {
  id: "tpl-kickoff",
  name: "Kickoff",
  description: null,
  type: "project",
  icon: null,
  color: null,
  hasFormFields: true,
  lastAppliedAt: null,
  sortOrder: 0,
  createdAt: "2024-01-01T12:00:00.000Z",
  updatedAt: "2024-01-02T12:00:00.000Z",
  team: null,
  inheritedFrom: { id: "tpl-parent", name: "Parent kickoff" },
  creator: null,
  templateData: JSON.stringify({
    name: "Kickoff: ",
    content: "## Goals\n\n## Milestones\n",
    labelIds: [],
    priority: 3,
    custom: { flag: true, nested: { depth: 2 } },
  }),
}

const BROKEN_TEMPLATE = {
  ...KICKOFF_TEMPLATE,
  id: "tpl-broken",
  name: "Broken",
  templateData: "[1, 2]",
}

async function withServer<T>(
  mocks: ConstructorParameters<typeof MockLinearServer>[0],
  run: () => Promise<T>,
): Promise<T> {
  const server = new MockLinearServer(mocks)
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    return await run()
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

const listMock = {
  queryName: "GetTemplates",
  response: {
    data: { templates: [BUG_TEMPLATE, KICKOFF_TEMPLATE, BROKEN_TEMPLATE] },
  },
}

await snapshotTest({
  name: "Template View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

await snapshotTest({
  name: "Template View Command - By ID Renders The Body",
  meta: import.meta,
  colors: false,
  args: [BUG_ID],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([
      {
        queryName: "GetTemplate",
        variables: { id: BUG_ID },
        response: { data: { template: BUG_TEMPLATE } },
      },
    ], () => viewCommand.parse())
  },
})

await snapshotTest({
  name: "Template View Command - By Name Shows Every Key",
  meta: import.meta,
  colors: false,
  args: ["kickoff"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([listMock], () => viewCommand.parse())
  },
})

await snapshotTest({
  name: "Template View Command - JSON Output Keeps templateData Encoded",
  meta: import.meta,
  colors: false,
  args: [BUG_ID, "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await withServer([
      {
        queryName: "GetTemplate",
        variables: { id: BUG_ID },
        response: { data: { template: BUG_TEMPLATE } },
      },
    ], () => viewCommand.parse())
  },
})

Deno.test("Template View Command - unknown name lists the available templates", async () => {
  const output = await withServer(
    [listMock],
    () => captureCommandError(() => viewCommand.parse(["Nope"])),
  )
  assertStringIncludes(
    output,
    "✗ Failed to view template: Template not found: Nope",
  )
  assertStringIncludes(
    output,
    'Available templates: "Broken", "Bug report", "Kickoff". Run `linear template list` to see every template.',
  )
})

Deno.test("Template View Command - ambiguous name asks for an ID", async () => {
  const output = await withServer(
    [
      {
        queryName: "GetTemplates",
        response: {
          data: {
            templates: [
              BUG_TEMPLATE,
              { ...BUG_TEMPLATE, id: "tpl-bug-workspace", team: null },
            ],
          },
        },
      },
    ],
    () => captureCommandError(() => viewCommand.parse(["bug REPORT"])),
  )
  assertStringIncludes(
    output,
    'Template name "bug REPORT" is ambiguous: it matches 2 templates',
  )
  assertStringIncludes(
    output,
    `Pass the template ID instead: ${BUG_ID} (issue, ENG), tpl-bug-workspace (issue, Workspace)`,
  )
})

Deno.test("Template View Command - malformed templateData is reported, not hidden", async () => {
  const output = await withServer(
    [listMock],
    () => captureCommandError(() => viewCommand.parse(["Broken"])),
  )
  assertStringIncludes(
    output,
    '✗ Failed to view template: Template data for "Broken" (tpl-broken) is not a JSON object',
  )
})
