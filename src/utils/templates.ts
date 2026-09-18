import { gql } from "../__codegen__/gql.ts"
import type { GetTemplatesQuery } from "../__codegen__/graphql.ts"
import { getGraphQLClient } from "./graphql.ts"
import { isLinearUuid } from "./linear.ts"
import {
  CliError,
  isClientError,
  NotFoundError,
  ValidationError,
} from "./errors.ts"

/**
 * Template types the CLI knows how to filter by and apply. Linear stores the
 * type as a plain string, so other types may exist; listing without a filter
 * keeps them.
 */
export const TEMPLATE_TYPES = ["issue", "project", "document"] as const
export type TemplateType = (typeof TEMPLATE_TYPES)[number]

// Both documents select the same fields so a template looks identical whether
// it was reached by name (through the workspace list) or by ID.
const GetTemplates = gql(`
  query GetTemplates {
    templates {
      id
      name
      description
      type
      icon
      color
      hasFormFields
      lastAppliedAt
      sortOrder
      createdAt
      updatedAt
      team {
        id
        key
        name
      }
      inheritedFrom {
        id
        name
      }
      creator {
        id
        name
      }
      templateData
    }
  }
`)

const GetTemplate = gql(`
  query GetTemplate($id: String!) {
    template(id: $id) {
      id
      name
      description
      type
      icon
      color
      hasFormFields
      lastAppliedAt
      sortOrder
      createdAt
      updatedAt
      team {
        id
        key
        name
      }
      inheritedFrom {
        id
        name
      }
      creator {
        id
        name
      }
      templateData
    }
  }
`)

export type Template = GetTemplatesQuery["templates"][number]

/**
 * Where a template must be usable from. A template qualifies when it is a
 * workspace template (no team) or belongs to one of the given teams, and its
 * type matches.
 */
export type TemplateScope = {
  type: TemplateType
  teamIds: readonly string[]
}

export function templateIsAvailableTo(
  template: Pick<Template, "team">,
  teamIds: readonly string[],
): boolean {
  return template.team == null || teamIds.includes(template.team.id)
}

export function templateScopeLabel(template: Pick<Template, "team">): string {
  return template.team == null ? "Workspace" : template.team.key
}

/** Every template in the workspace, team-scoped and workspace-level alike. */
export async function fetchTemplates(): Promise<Template[]> {
  const client = getGraphQLClient()
  const result = await client.request(GetTemplates)
  return result.templates
}

/**
 * One template by UUID. `template(id:)` is non-null, so Linear answers a
 * missing UUID with a GraphQL error ("No template found with id ...") rather
 * than a null field; translate that into a NotFoundError.
 */
export async function fetchTemplate(id: string): Promise<Template> {
  const client = getGraphQLClient()
  try {
    const result = await client.request(GetTemplate, { id })
    return result.template
  } catch (error) {
    if (
      isClientError(error) &&
      (error.response.errors ?? []).some((graphqlError) =>
        /no template found/i.test(graphqlError.message)
      )
    ) {
      throw new NotFoundError("Template", id, {
        suggestion: "Run `linear template list` to see every template.",
      })
    }
    throw error
  }
}

function describeType(type: string): string {
  const article = /^[aeiou]/i.test(type) ? "an" : "a"
  return `${article} ${type} template`
}

function wrongTypeError(
  template: Template,
  scope: TemplateScope,
): ValidationError {
  return new ValidationError(
    `Template "${template.name}" is ${describeType(template.type)}, not ${
      describeType(scope.type)
    }`,
    {
      suggestion:
        `Run \`linear template list --type ${scope.type}\` to see the ${scope.type} templates.`,
    },
  )
}

function otherTeamError(
  name: string,
  teamKeys: string[],
  scope: TemplateScope,
): ValidationError {
  const teams = teamKeys.join(", ")
  return new ValidationError(
    `Template "${name}" belongs to team${
      teamKeys.length === 1 ? "" : "s"
    } ${teams} and cannot be applied here`,
    {
      suggestion: `Pass --team ${
        teamKeys[0]
      }, or pick a workspace template or one from the target team with \`linear template list --type ${scope.type} --team <team>\`.`,
    },
  )
}

/** Why none of several same-named templates qualified, preferring the team mismatch: it is the actionable one. */
function scopeMismatchError(
  matches: Template[],
  scope: TemplateScope,
): ValidationError {
  const sameType = matches.filter((template) => template.type === scope.type)
  if (sameType.length > 0) {
    const teamKeys = [
      ...new Set(
        sameType.flatMap((template) =>
          template.team == null ? [] : [template.team.key]
        ),
      ),
    ]
    if (teamKeys.length > 0) {
      return otherTeamError(sameType[0].name, teamKeys, scope)
    }
  }
  return wrongTypeError(matches[0], scope)
}

function assertTemplateInScope(template: Template, scope: TemplateScope): void {
  if (template.type !== scope.type) {
    throw wrongTypeError(template, scope)
  }
  if (!templateIsAvailableTo(template, scope.teamIds)) {
    const team = template.team
    if (team == null) {
      throw new CliError(`Template "${template.name}" is not available here`)
    }
    throw otherTeamError(template.name, [team.key], scope)
  }
}

/**
 * Resolve a template reference (UUID or exact, case-insensitive name) to a
 * template. With a scope, only templates of that type that are available to
 * those teams qualify, so a same-named template of another team never causes
 * ambiguity. A reference that matches nothing errors with the names that would
 * have qualified; a reference that matches several errors with their IDs.
 */
export async function resolveTemplate(
  reference: string,
  scope?: TemplateScope,
): Promise<Template> {
  if (isLinearUuid(reference)) {
    const template = await fetchTemplate(reference)
    if (scope != null) {
      assertTemplateInScope(template, scope)
    }
    return template
  }

  const all = await fetchTemplates()
  const wanted = reference.toLowerCase()
  const byName = all.filter((template) =>
    template.name.toLowerCase() === wanted
  )
  const inScope = (template: Template): boolean =>
    scope == null ||
    (template.type === scope.type &&
      templateIsAvailableTo(template, scope.teamIds))
  const candidates = byName.filter(inScope)

  if (candidates.length === 1) {
    return candidates[0]
  }

  if (candidates.length === 0) {
    if (byName.length > 0 && scope != null) {
      // The name exists, just not for this type or team: say why.
      throw scopeMismatchError(byName, scope)
    }
    const what = scope == null ? "templates" : `${scope.type} templates`
    const names = [...new Set(all.filter(inScope).map((t) => t.name))]
      .sort((a, b) => a.localeCompare(b))
    const suggestion = names.length > 0
      ? `Available ${what}: ${
        names.map((name) => `"${name}"`).join(", ")
      }. Run \`linear template list\` to see every template.`
      : `No ${what} are available here. Run \`linear template list\` to see every template.`
    throw new NotFoundError("Template", reference, { suggestion })
  }

  throw new ValidationError(
    `Template name "${reference}" is ambiguous: it matches ${candidates.length} templates`,
    {
      suggestion: `Pass the template ID instead: ${
        candidates
          .map((t) => `${t.id} (${t.type}, ${templateScopeLabel(t)})`)
          .join(", ")
      }`,
    },
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

/**
 * The pre-filled attributes of a template. Linear returns `templateData` as a
 * JSON-encoded string inside the JSON scalar; accept an already-decoded object
 * too, and reject anything else loudly.
 */
export function parseTemplateData(
  template: Pick<Template, "id" | "name" | "templateData">,
): Record<string, unknown> {
  const raw: unknown = template.templateData
  let decoded: unknown = raw
  if (typeof raw === "string") {
    try {
      decoded = JSON.parse(raw)
    } catch (error) {
      throw new CliError(
        `Template data for "${template.name}" (${template.id}) is not valid JSON`,
        { cause: error },
      )
    }
  }
  if (!isRecord(decoded)) {
    throw new CliError(
      `Template data for "${template.name}" (${template.id}) is not a JSON object`,
    )
  }
  return decoded
}
