import { Command } from "@cliffy/command"
import { gql } from "../__codegen__/gql.ts"
import { getGraphQLClient } from "../utils/graphql.ts"
import { handleError, ValidationError } from "../utils/errors.ts"

const GetIssuesForPrBody = gql(`
  query GetIssuesForPrBody($ids: [ID!]) {
    issues(filter: { id: { in: $ids } }) {
      nodes {
        identifier
        title
        labels {
          nodes {
            name
          }
        }
      }
    }
  }
`)

const WORK_TYPE_LABELS = [
  "Unplanned",
  "Change",
  "Business",
  "Internal",
] as const

type WorkType = (typeof WORK_TYPE_LABELS)[number]

function parseIssueIdentifiers(raw: string): string[] {
  const ids = raw
    .split(",")
    .map((id) => id.trim())
    .filter(Boolean)

  if (ids.length === 0) {
    throw new ValidationError("At least one issue ID is required", {
      suggestion: "Pass a comma-separated list, e.g. --issues ALA-123,ALA-124",
    })
  }

  return ids
}

function inferWorkType(
  issues: Array<{ labels?: { nodes?: Array<{ name: string }> } | null }>,
): WorkType {
  for (const label of WORK_TYPE_LABELS) {
    if (
      issues.some((issue) =>
        issue.labels?.nodes?.some((issueLabel) => issueLabel.name === label)
      )
    ) {
      return label
    }
  }

  return "Internal"
}

function renderPrBody(
  issues: Array<{ identifier: string; title: string }>,
  workType: WorkType,
): string {
  const ticketLines = issues.map((issue) =>
    `- ${issue.identifier} — ${issue.title}`
  ).join("\n")

  return `## What this does
[Describe what changed in plain English — one paragraph, no jargon]

## Tickets addressed
${ticketLines}

## Work type
${workType}

## Review checklist
- [ ] Changes reviewed
- [ ] Ready to merge`
}

export const prBodyCommand = new Command()
  .name("pr-body")
  .description("Generate an Alavida PR body from Linear issues")
  .option(
    "--issues <issues:string>",
    "Comma-separated issue identifiers, e.g. ALA-123,ALA-124",
    { required: true },
  )
  .action(async ({ issues: rawIssues }) => {
    try {
      const issueIds = parseIssueIdentifiers(rawIssues)
      const client = getGraphQLClient()
      const result = await client.request(GetIssuesForPrBody, { ids: issueIds })
      const issues = result.issues?.nodes || []

      const issuesById = new Map(
        issues.map((issue) => [issue.identifier, issue] as const),
      )
      const orderedIssues = issueIds.map((issueId) => issuesById.get(issueId))
      const missingIssues = issueIds.filter((issueId) =>
        !issuesById.has(issueId)
      )

      if (missingIssues.length > 0) {
        throw new ValidationError(
          `Issue not found: ${missingIssues.join(", ")}`,
          {
            suggestion:
              "Check the issue identifiers and make sure they are accessible in the current Linear workspace.",
          },
        )
      }

      console.log(
        renderPrBody(
          orderedIssues.filter((issue): issue is NonNullable<typeof issue> =>
            Boolean(issue)
          ),
          inferWorkType(issues),
        ),
      )
    } catch (error) {
      handleError(error, "Failed to generate PR body")
    }
  })
