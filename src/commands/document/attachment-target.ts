import { gql } from "../../__codegen__/gql.ts"
import type { DocumentFilter } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getCycleIdByNameOrNumber,
  getTeamIdByKey,
  getTeamKey,
  isLinearUuid,
  resolveInitiativeId,
  resolveProjectId,
  resolveReleaseId,
} from "../../utils/linear.ts"
import {
  isClientError,
  isNotFoundError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

// A Linear document is attached to exactly one target. The API enforces
// "exactly one of initiativeId, teamId, issueId, releaseId, cycleId or
// projectId"; this module owns the CLI side of that rule so create, update,
// and list can't drift apart. Note: initiativeId/teamId/cycleId are marked
// [Internal] in Linear's schema but work with regular API keys (as issueId
// did before it became public).

export interface DocumentTargetOptions {
  project?: string
  issue?: string
  initiative?: string
  team?: string
  cycle?: string
  release?: string
}

export type DocumentTargetKind =
  | "project"
  | "issue"
  | "initiative"
  | "team"
  | "cycle"
  | "release"

export interface DocumentTarget {
  kind: DocumentTargetKind
  id: string
}

export type DocumentTargetSelector =
  | { kind: "project"; project: string }
  | { kind: "issue"; issue: string }
  | { kind: "initiative"; initiative: string }
  | { kind: "team"; team: string }
  | { kind: "cycle"; cycle: string; team?: string }
  | { kind: "release"; release: string }

export const TARGET_FLAGS_SUGGESTION =
  "Pass exactly one of --project, --issue, --initiative, --team, --cycle, or --release. " +
  "(--team combined with --cycle scopes the cycle lookup and does not count as a second target.)"

/**
 * Turn raw CLI option values into at most one target selector, validating
 * mutual exclusivity before any network work. `--team` together with
 * `--cycle` scopes the cycle lookup (like the issue commands) rather than
 * acting as a second target.
 */
export function parseDocumentTargetOptions(
  options: DocumentTargetOptions,
  requirement: "exactly-one",
): DocumentTargetSelector
export function parseDocumentTargetOptions(
  options: DocumentTargetOptions,
  requirement: "at-most-one",
): DocumentTargetSelector | undefined
export function parseDocumentTargetOptions(
  options: DocumentTargetOptions,
  requirement: "exactly-one" | "at-most-one",
): DocumentTargetSelector | undefined {
  const selectors: DocumentTargetSelector[] = []
  if (options.project != null) {
    selectors.push({ kind: "project", project: options.project })
  }
  if (options.issue != null) {
    selectors.push({ kind: "issue", issue: options.issue })
  }
  if (options.initiative != null) {
    selectors.push({ kind: "initiative", initiative: options.initiative })
  }
  if (options.cycle != null) {
    selectors.push({ kind: "cycle", cycle: options.cycle, team: options.team })
  } else if (options.team != null) {
    selectors.push({ kind: "team", team: options.team })
  }
  if (options.release != null) {
    selectors.push({ kind: "release", release: options.release })
  }

  if (selectors.length > 1) {
    const flags = selectors.map((s) => `--${s.kind}`).join(", ")
    throw new ValidationError(
      `Only one attachment target may be set (got ${flags})`,
      { suggestion: TARGET_FLAGS_SUGGESTION },
    )
  }
  if (selectors.length === 0 && requirement === "exactly-one") {
    throw new ValidationError("A document attachment target is required", {
      suggestion: TARGET_FLAGS_SUGGESTION,
    })
  }
  return selectors[0]
}

const GetIssueForDocumentTarget = gql(/* GraphQL */ `
  query GetIssueForDocumentTarget($id: String!) {
    issue(id: $id) {
      id
    }
  }
`)

async function resolveIssueId(input: string): Promise<string> {
  const client = getGraphQLClient()
  const id = isLinearUuid(input) ? input : input.toUpperCase()
  try {
    const result = await client.request(GetIssueForDocumentTarget, { id })
    if (result.issue) {
      return result.issue.id
    }
  } catch (error) {
    if (isClientError(error) && isNotFoundError(error)) {
      throw new NotFoundError("Issue", input, {
        suggestion: "Provide a valid issue identifier (e.g., TC-123) or UUID.",
      })
    }
    throw error
  }
  throw new NotFoundError("Issue", input, {
    suggestion: "Provide a valid issue identifier (e.g., TC-123) or UUID.",
  })
}

async function resolveTeamIdStrict(teamKey: string): Promise<string> {
  const teamId = await getTeamIdByKey(teamKey.toUpperCase())
  if (!teamId) {
    throw new NotFoundError("Team", teamKey, {
      suggestion: "Pass a team key, e.g. --team ENG.",
    })
  }
  return teamId
}

async function resolveCycleScopeTeamId(explicitTeam?: string): Promise<string> {
  // An explicitly passed team must resolve or error — never fall back to the
  // configured default team when explicit input is invalid.
  if (explicitTeam != null) {
    return await resolveTeamIdStrict(explicitTeam)
  }
  const configTeam = getTeamKey()
  if (configTeam != null) {
    return await resolveTeamIdStrict(configTeam)
  }
  throw new ValidationError("--cycle requires a team to look the cycle up in", {
    suggestion: "Pass --team <key> or configure a default team.",
  })
}

/**
 * Resolve a parsed selector to the target's UUID.
 */
export async function resolveDocumentTarget(
  selector: DocumentTargetSelector,
): Promise<DocumentTarget> {
  switch (selector.kind) {
    case "project":
      return { kind: "project", id: await resolveProjectId(selector.project) }
    case "issue":
      return { kind: "issue", id: await resolveIssueId(selector.issue) }
    case "initiative":
      return {
        kind: "initiative",
        id: await resolveInitiativeId(selector.initiative),
      }
    case "team":
      return { kind: "team", id: await resolveTeamIdStrict(selector.team) }
    case "cycle": {
      const teamId = await resolveCycleScopeTeamId(selector.team)
      return {
        kind: "cycle",
        id: await getCycleIdByNameOrNumber(selector.cycle, teamId),
      }
    }
    case "release":
      return { kind: "release", id: await resolveReleaseId(selector.release) }
    default:
      throw selector satisfies never
  }
}

export type DocumentTargetInput =
  | { projectId: string }
  | { issueId: string }
  | { initiativeId: string }
  | { teamId: string }
  | { cycleId: string }
  | { releaseId: string }

/**
 * Map a resolved target to the single DocumentCreateInput/DocumentUpdateInput
 * ID field it sets.
 */
export function toDocumentTargetInput(
  target: DocumentTarget,
): DocumentTargetInput {
  switch (target.kind) {
    case "project":
      return { projectId: target.id }
    case "issue":
      return { issueId: target.id }
    case "initiative":
      return { initiativeId: target.id }
    case "team":
      return { teamId: target.id }
    case "cycle":
      return { cycleId: target.id }
    case "release":
      return { releaseId: target.id }
    default:
      throw target.kind satisfies never
  }
}

/**
 * Map a resolved target to the single DocumentFilter relation fragment it
 * filters by.
 */
export function toDocumentTargetFilter(target: DocumentTarget): DocumentFilter {
  switch (target.kind) {
    case "project":
      return { project: { id: { eq: target.id } } }
    case "issue":
      return { issue: { id: { eq: target.id } } }
    case "initiative":
      return { initiative: { id: { eq: target.id } } }
    case "team":
      return { team: { id: { eq: target.id } } }
    case "cycle":
      return { cycle: { id: { eq: target.id } } }
    case "release":
      return { release: { id: { eq: target.id } } }
    default:
      throw target.kind satisfies never
  }
}
