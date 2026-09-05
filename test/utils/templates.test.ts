import { assertEquals, assertRejects } from "@std/assert"
import {
  parseTemplateData,
  resolveTemplate,
} from "../../src/utils/templates.ts"
import {
  CliError,
  NotFoundError,
  ValidationError,
} from "../../src/utils/errors.ts"
import { setupMockLinearServer } from "./test-helpers.ts"

const ENG = { id: "team-eng-id", key: "ENG", name: "Engineering" }
const OPS = { id: "team-ops-id", key: "OPS", name: "Operations" }

function template(
  id: string,
  name: string,
  type: string,
  team: { id: string; key: string; name: string } | null,
) {
  return {
    id,
    name,
    description: null,
    type,
    icon: null,
    color: null,
    hasFormFields: false,
    lastAppliedAt: null,
    sortOrder: 0,
    createdAt: "2024-01-01T00:00:00.000Z",
    updatedAt: "2024-01-01T00:00:00.000Z",
    team,
    inheritedFrom: null,
    creator: null,
    templateData: "{}",
  }
}

const BUG_ENG = "11111111-1111-4111-8111-111111111111"
const BUG_OPS = "22222222-2222-4222-8222-222222222222"
const BUG_WORKSPACE = "33333333-3333-4333-8333-333333333333"
const FEATURE_ENG = "44444444-4444-4444-8444-444444444444"
const FEATURE_OPS = "55555555-5555-4555-8555-555555555555"
const KICKOFF = "66666666-6666-4666-8666-666666666666"
const ROADMAP_OPS = "77777777-7777-4777-8777-777777777777"
const DESIGN_ENG = "88888888-8888-4888-8888-888888888888"
const RETRO_WORKSPACE_PROJECT = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
const RETRO_OPS_ISSUE = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
const MISSING = "99999999-9999-4999-8999-999999999999"

const TEMPLATES = [
  template(BUG_ENG, "Bug report", "issue", ENG),
  template(BUG_OPS, "Bug report", "issue", OPS),
  template(BUG_WORKSPACE, "Bug report", "issue", null),
  template(FEATURE_ENG, "Feature request", "issue", ENG),
  template(FEATURE_OPS, "Feature request", "issue", OPS),
  template(KICKOFF, "Kickoff", "project", null),
  template(ROADMAP_OPS, "Roadmap", "project", OPS),
  template(DESIGN_ENG, "Design doc", "document", ENG),
  // Same name, excluded for different reasons: the API order must not decide
  // which reason is reported.
  template(RETRO_WORKSPACE_PROJECT, "Retro", "project", null),
  template(RETRO_OPS_ISSUE, "Retro", "issue", OPS),
]

Deno.test("resolveTemplate", async (t) => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetTemplates",
      response: { data: { templates: TEMPLATES } },
    },
    ...TEMPLATES.map((entry) => ({
      queryName: "GetTemplate",
      variables: { id: entry.id },
      response: { data: { template: entry } },
    })),
    {
      queryName: "GetTemplate",
      variables: { id: MISSING },
      response: {
        errors: [{
          message: `No template found with id ${MISSING}`,
          path: ["template"],
        }],
        data: null,
      },
    },
  ])

  try {
    await t.step(
      "matches a name case-insensitively and filters by scope before judging ambiguity",
      async () => {
        const resolved = await resolveTemplate("feature REQUEST", {
          type: "issue",
          teamIds: [ENG.id],
        })
        assertEquals(resolved.id, FEATURE_ENG)
      },
    )

    await t.step(
      "a name that is still ambiguous within scope errors with the IDs",
      async () => {
        const error = await assertRejects(
          () =>
            resolveTemplate("Bug report", { type: "issue", teamIds: [ENG.id] }),
          ValidationError,
          'Template name "Bug report" is ambiguous: it matches 2 templates',
        )
        assertEquals(
          error.suggestion,
          `Pass the template ID instead: ${BUG_ENG} (issue, ENG), ${BUG_WORKSPACE} (issue, Workspace)`,
        )
      },
    )

    await t.step(
      "without a scope every same-named template counts",
      async () => {
        await assertRejects(
          () => resolveTemplate("Feature request"),
          ValidationError,
          "matches 2 templates",
        )
      },
    )

    await t.step("a workspace template is available to any team", async () => {
      const resolved = await resolveTemplate("Kickoff", {
        type: "project",
        teamIds: [ENG.id],
      })
      assertEquals(resolved.id, KICKOFF)
    })

    await t.step(
      "a team template is available when any of the scope's teams owns it",
      async () => {
        const resolved = await resolveTemplate("Roadmap", {
          type: "project",
          teamIds: [ENG.id, OPS.id],
        })
        assertEquals(resolved.id, ROADMAP_OPS)
      },
    )

    await t.step("a name of the wrong type says which type it is", async () => {
      const error = await assertRejects(
        () => resolveTemplate("Kickoff", { type: "issue", teamIds: [ENG.id] }),
        ValidationError,
        'Template "Kickoff" is a project template, not an issue template',
      )
      assertEquals(
        error.suggestion,
        "Run `linear template list --type issue` to see the issue templates.",
      )
    })

    await t.step(
      "a name owned by another team says which team owns it",
      async () => {
        await assertRejects(
          () =>
            resolveTemplate("Roadmap", { type: "project", teamIds: [ENG.id] }),
          ValidationError,
          'Template "Roadmap" belongs to team OPS and cannot be applied here',
        )
      },
    )

    await t.step(
      "when same-named templates miss for different reasons, the team mismatch is reported",
      async () => {
        await assertRejects(
          () => resolveTemplate("Retro", { type: "issue", teamIds: [ENG.id] }),
          ValidationError,
          'Template "Retro" belongs to team OPS and cannot be applied here',
        )
        await assertRejects(
          () =>
            resolveTemplate("Retro", { type: "document", teamIds: [ENG.id] }),
          ValidationError,
          'Template "Retro" is a project template, not a document template',
        )
      },
    )

    await t.step(
      "an unknown name lists the names that would have qualified",
      async () => {
        const error = await assertRejects(
          () => resolveTemplate("Nope", { type: "issue", teamIds: [OPS.id] }),
          NotFoundError,
          "Template not found: Nope",
        )
        assertEquals(
          error.suggestion,
          'Available issue templates: "Bug report", "Feature request", "Retro". Run `linear template list` to see every template.',
        )
      },
    )

    await t.step("an unknown name with nothing in scope says so", async () => {
      const error = await assertRejects(
        () => resolveTemplate("Nope", { type: "document", teamIds: [OPS.id] }),
        NotFoundError,
      )
      assertEquals(
        error.suggestion,
        "No document templates are available here. Run `linear template list` to see every template.",
      )
    })

    await t.step(
      "a UUID is fetched directly and checked against the scope",
      async () => {
        const resolved = await resolveTemplate(FEATURE_OPS, {
          type: "issue",
          teamIds: [OPS.id],
        })
        assertEquals(resolved.name, "Feature request")
        await assertRejects(
          () =>
            resolveTemplate(FEATURE_OPS, { type: "issue", teamIds: [ENG.id] }),
          ValidationError,
          "belongs to team OPS",
        )
        await assertRejects(
          () =>
            resolveTemplate(DESIGN_ENG, { type: "issue", teamIds: [ENG.id] }),
          ValidationError,
          "is a document template, not an issue template",
        )
      },
    )

    await t.step("a missing UUID becomes a not-found error", async () => {
      await assertRejects(
        () => resolveTemplate(MISSING),
        NotFoundError,
        `Template not found: ${MISSING}`,
      )
    })
  } finally {
    await cleanup()
  }
})

Deno.test("parseTemplateData decodes the JSON-encoded string and rejects other shapes", () => {
  const base = { id: "tpl", name: "Bug report" }
  assertEquals(
    parseTemplateData({
      ...base,
      templateData: '{"title":"Bug: ","priority":2}',
    }),
    { title: "Bug: ", priority: 2 },
  )
  assertEquals(
    parseTemplateData({ ...base, templateData: { title: "already decoded" } }),
    { title: "already decoded" },
  )
  assertRejectsSync(
    () => parseTemplateData({ ...base, templateData: "not json" }),
    "is not valid JSON",
  )
  assertRejectsSync(
    () => parseTemplateData({ ...base, templateData: "[1,2]" }),
    "is not a JSON object",
  )
  assertRejectsSync(
    () => parseTemplateData({ ...base, templateData: 42 }),
    "is not a JSON object",
  )
})

function assertRejectsSync(fn: () => unknown, messageIncludes: string) {
  let caught: unknown
  try {
    fn()
  } catch (error) {
    caught = error
  }
  if (!(caught instanceof CliError)) {
    throw new Error(`Expected a CliError, got ${String(caught)}`)
  }
  if (!caught.userMessage.includes(messageIncludes)) {
    throw new Error(
      `Expected "${caught.userMessage}" to include "${messageIncludes}"`,
    )
  }
}
