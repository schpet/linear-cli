import { assertStringIncludes } from "@std/assert"
import { unarchiveCommand } from "../../../src/commands/initiative/initiative-unarchive.ts"
import { setupMockLinearServer } from "../../utils/test-helpers.ts"

/**
 * An archived initiative is only visible to lookups that ask for archived
 * records. A URL's slug is resolved strictly — never by name — and that strict
 * lookup once omitted `includeArchived`, so the one command whose whole job is
 * archived initiatives could not find one by URL. The slug mock below only
 * answers when `includeArchived` is true.
 */
Deno.test("initiative unarchive finds an archived initiative by its URL", async () => {
  const previousWorkspace = Deno.env.get("LINEAR_WORKSPACE")
  Deno.env.set("LINEAR_WORKSPACE", "url-test-workspace")
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveInitiativeBySlug",
      variables: { slugId: "43bc13e544d9", includeArchived: true },
      response: {
        data: {
          initiatives: {
            nodes: [{ id: "c44f9540-2e02-42c9-bec9-82e4e9529bc0" }],
          },
        },
      },
    },
    {
      queryName: "GetInitiativeForUnarchive",
      variables: { id: "c44f9540-2e02-42c9-bec9-82e4e9529bc0" },
      response: {
        data: {
          initiatives: {
            nodes: [{
              id: "c44f9540-2e02-42c9-bec9-82e4e9529bc0",
              slugId: "43bc13e544d9",
              name: "Archived initiative",
              archivedAt: "2026-09-20T00:00:00Z",
            }],
          },
        },
      },
    },
    {
      queryName: "UnarchiveInitiative",
      variables: { id: "c44f9540-2e02-42c9-bec9-82e4e9529bc0" },
      response: {
        data: {
          initiativeUnarchive: {
            success: true,
            entity: {
              id: "c44f9540-2e02-42c9-bec9-82e4e9529bc0",
              slugId: "43bc13e544d9",
              name: "Archived initiative",
              url:
                "https://linear.app/url-test-workspace/initiative/archived-initiative-43bc13e544d9",
            },
          },
        },
      },
    },
  ])

  const originalLog = console.log
  const output: string[] = []
  console.log = (...args: unknown[]) => {
    output.push(args.map(String).join(" "))
  }
  try {
    await unarchiveCommand.parse([
      "https://linear.app/url-test-workspace/initiative/archived-initiative-43bc13e544d9",
      "--force",
    ])
  } finally {
    console.log = originalLog
    await cleanup()
    if (previousWorkspace == null) {
      Deno.env.delete("LINEAR_WORKSPACE")
    } else {
      Deno.env.set("LINEAR_WORKSPACE", previousWorkspace)
    }
  }

  assertStringIncludes(
    output.join("\n"),
    "Unarchived initiative: Archived initiative",
  )
})
