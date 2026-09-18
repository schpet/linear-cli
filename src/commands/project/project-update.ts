import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { ProjectUpdateInput } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import type { GraphQLClient } from "graphql-request"
import {
  getProjectLabelIdByName,
  isLinearUuid,
  lookupUserId,
  resolveInitiativeId,
  resolveProjectId,
  resolveTeams,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"
import {
  PROJECT_DESCRIPTION_MAX_LENGTH,
  resolveProjectDescription,
} from "./project-description.ts"
import { resolveProjectContent } from "./project-create.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"

const UpdateProject = gql(`
  mutation UpdateProject($id: String!, $input: ProjectUpdateInput!) {
    projectUpdate(id: $id, input: $input) {
      success
      project {
        id
        slugId
        name
        description
        url
        updatedAt
      }
    }
  }
`)

const GetProjectStatuses = gql(`
  query GetProjectStatuses {
    projectStatuses {
      nodes {
        id
        name
        type
      }
    }
  }
`)

// ProjectUpdateInput only has replace-style teamIds/labelIds, so --add-*/
// --remove-* read the current set, compute the new one, and send the full
// set. Each connection is paginated: a project with more than one page must
// never have its later pages silently dropped from the replacement set.
const GetProjectTeamsForUpdate = gql(`
  query GetProjectTeamsForUpdate($id: String!, $after: String) {
    project(id: $id) {
      teams(first: 250, after: $after) {
        nodes {
          id
          key
          name
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

const GetProjectLabelsForUpdate = gql(`
  query GetProjectLabelsForUpdate($id: String!, $after: String) {
    project(id: $id) {
      labels(first: 250, after: $after) {
        nodes {
          id
          name
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

// Initiative membership is a join row (InitiativeToProject) with no field on
// ProjectUpdateInput; the row id is what the delete mutation needs. name/url
// are selected so an initiative-only update can print the success line
// without sending an empty projectUpdate.
const GetProjectInitiativeLinksForUpdate = gql(`
  query GetProjectInitiativeLinksForUpdate($id: String!, $after: String) {
    project(id: $id) {
      id
      name
      url
      initiativeToProjects(first: 250, after: $after) {
        nodes {
          id
          initiative {
            id
            name
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

// resolveInitiativeId passes a UUID through unchecked. Links are deleted
// before new ones are created, so an unknown UUID must be caught here or a
// replace would drop the old links and then fail to add the new one.
const GetInitiativeByIdForUpdate = gql(`
  query GetInitiativeByIdForUpdate($id: ID!) {
    initiatives(filter: { id: { eq: $id } }) {
      nodes {
        id
        name
      }
    }
  }
`)

const AddProjectToInitiativeForUpdate = gql(`
  mutation AddProjectToInitiativeForUpdate($input: InitiativeToProjectCreateInput!) {
    initiativeToProjectCreate(input: $input) {
      success
    }
  }
`)

const RemoveProjectFromInitiativeForUpdate = gql(`
  mutation RemoveProjectFromInitiativeForUpdate($id: String!) {
    initiativeToProjectDelete(id: $id) {
      success
    }
  }
`)

const STATUS_TYPE_MAPPING: Record<string, string> = {
  "planned": "planned",
  "in progress": "started",
  "started": "started",
  "paused": "paused",
  "completed": "completed",
  "canceled": "canceled",
  "backlog": "backlog",
}

type Page<T> = {
  nodes: T[]
  pageInfo: { hasNextPage: boolean; endCursor?: string | null }
}

/** Follow a connection's cursor until every page has been read. */
async function fetchAllPages<T extends { id: string }>(
  fetchPage: (after: string | null) => Promise<Page<T>>,
): Promise<T[]> {
  const nodes: T[] = []
  const seen = new Set<string>()
  let after: string | null = null
  while (true) {
    const page: Page<T> = await fetchPage(after)
    for (const node of page.nodes) {
      if (seen.has(node.id)) continue
      seen.add(node.id)
      nodes.push(node)
    }
    if (!page.pageInfo.hasNextPage) return nodes
    if (page.pageInfo.endCursor == null) {
      throw new CliError(
        "Linear reported another page of results but returned no cursor to fetch it",
      )
    }
    if (page.pageInfo.endCursor === after) {
      throw new CliError(
        "Linear reported another page of results but returned the same cursor again",
      )
    }
    after = page.pageInfo.endCursor
  }
}

/** A user-supplied reference resolved to an id, keeping the reference for messages. */
type ResolvedRef = { id: string; label: string }

/**
 * Apply --add/--remove to a collection: current order is kept, removed ids
 * are dropped, added ids not already present are appended in flag order. A
 * removal that is not in the current set throws before anything is sent.
 */
function applyCollectionEdit(opts: {
  current: readonly string[]
  add: readonly ResolvedRef[]
  remove: readonly ResolvedRef[]
  onMissing: (ref: ResolvedRef) => Error
}): string[] {
  const currentSet = new Set(opts.current)
  for (const ref of opts.remove) {
    if (!currentSet.has(ref.id)) throw opts.onMissing(ref)
  }
  const removeIds = new Set(opts.remove.map((r) => r.id))
  const result = opts.current.filter((id) => !removeIds.has(id))
  for (const ref of opts.add) {
    if (!result.includes(ref.id)) result.push(ref.id)
  }
  return result
}

function rejectAddRemoveOverlap(
  kind: "team" | "label" | "initiative",
  add: readonly ResolvedRef[],
  remove: readonly ResolvedRef[],
): void {
  const removeIds = new Set(remove.map((r) => r.id))
  if (add.some((r) => removeIds.has(r.id))) {
    throw new ValidationError(
      `Cannot add and remove the same ${kind} in one update`,
      {
        suggestion:
          `Remove the duplicate ${kind} from either --add-${kind} or --remove-${kind}.`,
      },
    )
  }
}

function rejectReplaceWithIncremental(
  kind: "team" | "label" | "initiative",
  replace: readonly string[] | undefined,
  add: readonly string[] | undefined,
  remove: readonly string[] | undefined,
): void {
  if (replace != null && (add != null || remove != null)) {
    throw new ValidationError(
      `Cannot combine --${kind} with --add-${kind} or --remove-${kind}`,
      {
        suggestion:
          `--${kind} replaces the project's entire ${kind} set. Use it alone to set the exact set, or use --add-${kind}/--remove-${kind} alone to change it incrementally.`,
      },
    )
  }
}

/** Resolve project label names to ids, deduped by id, erroring on unknown names. */
async function resolveProjectLabels(
  names: readonly string[],
): Promise<ResolvedRef[]> {
  const resolved: ResolvedRef[] = []
  for (const name of names) {
    const id = await getProjectLabelIdByName(name)
    if (!id) {
      throw new NotFoundError("Project label", name)
    }
    if (!resolved.some((r) => r.id === id)) {
      resolved.push({ id, label: name })
    }
  }
  return resolved
}

async function resolveInitiatives(
  client: GraphQLClient,
  references: readonly string[],
): Promise<ResolvedRef[]> {
  const resolved: ResolvedRef[] = []
  for (const reference of references) {
    let ref: ResolvedRef
    if (isLinearUuid(reference)) {
      const data = await client.request(GetInitiativeByIdForUpdate, {
        id: reference,
      })
      const initiative = data.initiatives.nodes[0]
      if (initiative == null) {
        throw new NotFoundError("Initiative", reference, {
          suggestion:
            "Pass an initiative UUID, slug ID, or exact initiative name.",
        })
      }
      ref = { id: initiative.id, label: initiative.name }
    } else {
      ref = { id: await resolveInitiativeId(reference), label: reference }
    }
    if (!resolved.some((r) => r.id === ref.id)) {
      resolved.push(ref)
    }
  }
  return resolved
}

type InitiativeChange =
  | { kind: "add"; initiativeId: string; label: string }
  | { kind: "remove"; linkId: string; initiativeId: string; label: string }

function describeInitiativeChange(change: InitiativeChange): string {
  return change.kind === "add"
    ? `added "${change.label}"`
    : `removed "${change.label}"`
}

// By UUID: initiative names are not unique and the resolver rejects an
// ambiguous name, so a name here could make the suggested command unrunnable.
function initiativeChangeFlag(change: InitiativeChange): string {
  return change.kind === "add"
    ? `--add-initiative ${change.initiativeId}`
    : `--remove-initiative ${change.initiativeId}`
}

/**
 * Apply initiative link changes one mutation at a time. Linear has no
 * transaction across join-row mutations, so a failure part-way leaves earlier
 * changes applied; the error says exactly which, and which are still pending,
 * instead of implying a rollback or that the identical command is safe to
 * re-run (a completed --remove-initiative would fail its presence check).
 * A request that throws may have been committed before the reply was lost,
 * so that change is reported as unknown rather than as not applied.
 */
async function applyInitiativeChanges(
  client: GraphQLClient,
  projectId: string,
  changes: readonly InitiativeChange[],
  // Work already committed before these changes started (the projectUpdate
  // for the other fields), so the failure report never reads as "no effect".
  priorApplied: string | undefined,
): Promise<void> {
  const fail = (
    applied: number,
    outcome: "rejected" | "unknown",
    cause: unknown,
  ): CliError => {
    const done = [
      ...priorApplied != null ? [priorApplied] : [],
      ...changes.slice(0, applied).map(describeInitiativeChange),
    ]
    const current = changes[applied]
    const rest = changes.slice(applied + 1)
    const unknownText = outcome === "unknown"
      ? ` Unknown (the request failed before Linear answered): ${
        describeInitiativeChange(current)
      }.`
      : ""
    const notApplied = outcome === "unknown" ? rest : [current, ...rest]
    const notAppliedText = notApplied.length > 0
      ? ` Not applied: ${notApplied.map(describeInitiativeChange).join(", ")}.`
      : ""
    const remaining = [current, ...rest].map(initiativeChangeFlag).join(" ")
    return new CliError(
      `Failed to update project initiatives after ${applied} of ${changes.length} changes; earlier changes were not rolled back. Applied: ${
        done.length > 0 ? done.join(", ") : "none"
      }.${unknownText}${notAppliedText}`,
      {
        suggestion: outcome === "unknown"
          ? `Check the project's initiatives, then re-run with only the remaining changes (${remaining}), or use --initiative to set the exact set.`
          : `Re-run with only the remaining changes (${remaining}), or use --initiative to set the exact set.`,
        cause,
      },
    )
  }

  let applied = 0
  for (const change of changes) {
    let success: boolean
    try {
      if (change.kind === "add") {
        const result = await client.request(AddProjectToInitiativeForUpdate, {
          input: { initiativeId: change.initiativeId, projectId },
        })
        success = result.initiativeToProjectCreate.success
      } else {
        const result = await client.request(
          RemoveProjectFromInitiativeForUpdate,
          { id: change.linkId },
        )
        success = result.initiativeToProjectDelete.success
      }
    } catch (cause) {
      throw fail(applied, "unknown", cause)
    }
    if (!success) {
      throw fail(
        applied,
        "rejected",
        new CliError(
          `Linear reported failure for initiative "${change.label}"`,
        ),
      )
    }
    applied++
  }
}

export const updateCommand = new Command()
  .name("update")
  .description(withMarkdownHint("Update a Linear project"))
  .arguments("<projectId:string>")
  .option("-n, --name <name:string>", "Project name")
  .option(
    "-d, --description <description:string>",
    `Project description (max ${PROJECT_DESCRIPTION_MAX_LENGTH} characters, enforced by Linear's API)`,
  )
  .option(
    "-f, --description-file <path:string>",
    `Read project description from file (still subject to the ${PROJECT_DESCRIPTION_MAX_LENGTH}-character API limit)`,
  )
  .option("--content <markdown:string>", "Project overview markdown")
  .option(
    "--content-file <path:string>",
    "Read project overview markdown from a file",
  )
  .option(
    "-s, --status <status:string>",
    "Status (planned, started, paused, completed, canceled, backlog)",
  )
  .option(
    "-l, --lead <lead:string>",
    "Project lead (username, email, or @me). Use --clear-lead to remove it",
  )
  .option(
    "--clear-lead",
    "Remove the project's lead (cannot be combined with --lead)",
  )
  .option(
    "--start-date <startDate:string>",
    "Start date (YYYY-MM-DD). Use --clear-start-date to remove it",
  )
  .option(
    "--clear-start-date",
    "Remove the project's start date (cannot be combined with --start-date)",
  )
  .option(
    "--target-date <targetDate:string>",
    "Target date (YYYY-MM-DD). Use --clear-target-date to remove it",
  )
  .option(
    "--clear-target-date",
    "Remove the project's target date (cannot be combined with --target-date)",
  )
  .option(
    "-t, --team <team:string>",
    "Team key, name, or ID; replaces the project's entire team set. May be repeated. Use --add-team/--remove-team to change teams incrementally.",
    { collect: true },
  )
  .option(
    "--add-team <team:string>",
    "Add a team to the project, keeping its existing teams. May be repeated.",
    { collect: true },
  )
  .option(
    "--remove-team <team:string>",
    "Remove a team from the project, keeping its other teams. May be repeated.",
    { collect: true },
  )
  .option(
    "--label <label:string>",
    "Project label; replaces the project's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.",
    { collect: true },
  )
  .option(
    "--add-label <label:string>",
    "Add a label to the project, keeping its existing labels. May be repeated.",
    { collect: true },
  )
  .option(
    "--remove-label <label:string>",
    "Remove a label from the project, keeping its other labels (does not delete the label). May be repeated.",
    { collect: true },
  )
  .option(
    "--initiative <initiative:string>",
    "Initiative ID, slug, or name; replaces the project's entire initiative set. May be repeated. Use --add-initiative/--remove-initiative to change initiatives incrementally.",
    { collect: true },
  )
  .option(
    "--add-initiative <initiative:string>",
    "Add the project to an initiative, keeping its existing initiatives. May be repeated.",
    { collect: true },
  )
  .option(
    "--remove-initiative <initiative:string>",
    "Remove the project from an initiative, keeping its other initiatives (does not delete the initiative). May be repeated.",
    { collect: true },
  )
  .action(
    async (
      {
        name,
        description,
        descriptionFile,
        content,
        contentFile,
        status,
        lead,
        clearLead,
        startDate,
        clearStartDate,
        targetDate,
        clearTargetDate,
        team: teams,
        addTeam,
        removeTeam,
        label: labels,
        addLabel,
        removeLabel,
        initiative: initiatives,
        addInitiative,
        removeInitiative,
      },
      projectId,
    ) => {
      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null

      try {
        // Null checks, not truthiness: an empty --content-file is still an
        // explicit value to forward, so it must count as an update.
        if (
          !name && description == null && descriptionFile == null &&
          content == null && contentFile == null && !status &&
          !lead && !clearLead && !startDate && !clearStartDate &&
          !targetDate && !clearTargetDate &&
          teams == null && addTeam == null && removeTeam == null &&
          labels == null && addLabel == null && removeLabel == null &&
          initiatives == null && addInitiative == null &&
          removeInitiative == null
        ) {
          throw new ValidationError(
            "At least one update option must be provided",
            {
              suggestion:
                "Use --name, --description, --description-file, --content, --content-file, --status, --lead, --clear-lead, --start-date, --clear-start-date, --target-date, --clear-target-date, --team, --add-team, --remove-team, --label, --add-label, --remove-label, --initiative, --add-initiative, or --remove-initiative",
            },
          )
        }

        if (clearLead && lead != null) {
          throw new ValidationError(
            "Cannot specify both --lead and --clear-lead",
            {
              suggestion:
                "Use --lead <user> to set a lead, or --clear-lead on its own to remove it.",
            },
          )
        }

        if (clearStartDate && startDate != null) {
          throw new ValidationError(
            "Cannot specify both --start-date and --clear-start-date",
            {
              suggestion:
                "Use --start-date <date> to set a start date, or --clear-start-date on its own to remove it.",
            },
          )
        }

        if (clearTargetDate && targetDate != null) {
          throw new ValidationError(
            "Cannot specify both --target-date and --clear-target-date",
            {
              suggestion:
                "Use --target-date <date> to set a target date, or --clear-target-date on its own to remove it.",
            },
          )
        }

        rejectReplaceWithIncremental("team", teams, addTeam, removeTeam)
        rejectReplaceWithIncremental("label", labels, addLabel, removeLabel)
        rejectReplaceWithIncremental(
          "initiative",
          initiatives,
          addInitiative,
          removeInitiative,
        )

        for (
          const label of [
            ...labels ?? [],
            ...addLabel ?? [],
            ...removeLabel ?? [],
          ]
        ) {
          if (label.trim() === "") {
            throw new ValidationError("Project label cannot be empty", {
              suggestion: 'Provide a label name, e.g. --label "My Label".',
            })
          }
        }

        const resolvedDescription = await resolveProjectDescription(
          description,
          descriptionFile,
        )
        const resolvedContent = await resolveProjectContent(
          content,
          contentFile,
        )

        if (startDate && !/^\d{4}-\d{2}-\d{2}$/.test(startDate)) {
          throw new ValidationError("Start date must be in YYYY-MM-DD format")
        }

        if (targetDate && !/^\d{4}-\d{2}-\d{2}$/.test(targetDate)) {
          throw new ValidationError("Target date must be in YYYY-MM-DD format")
        }

        spinner?.start()
        const client = getGraphQLClient()
        const resolvedId = await resolveProjectId(projectId)

        const input: ProjectUpdateInput = {}

        if (name) input.name = name
        if (resolvedDescription != null) input.description = resolvedDescription
        if (resolvedContent != null) input.content = resolvedContent
        // Clearing a field requires an explicit flag; never set a field to
        // null implicitly.
        if (clearStartDate) {
          input.startDate = null
        } else if (startDate) {
          input.startDate = startDate
        }
        if (clearTargetDate) {
          input.targetDate = null
        } else if (targetDate) {
          input.targetDate = targetDate
        }

        if (status) {
          const statusLower = status.toLowerCase()
          const apiStatusType = STATUS_TYPE_MAPPING[statusLower]
          if (!apiStatusType) {
            spinner?.stop()
            throw new ValidationError(`Invalid status: ${status}`, {
              suggestion:
                "Valid values: planned, started, paused, completed, canceled, backlog",
            })
          }
          const statusResult = await client.request(GetProjectStatuses)
          const projectStatuses = statusResult.projectStatuses?.nodes || []
          const matchingStatus = projectStatuses.find(
            (s: { type: string }) => s.type === apiStatusType,
          )
          if (!matchingStatus) {
            spinner?.stop()
            throw new NotFoundError("Project status", apiStatusType)
          }
          input.statusId = matchingStatus.id
        }

        if (clearLead) {
          input.leadId = null
        } else if (lead) {
          const leadId = await lookupUserId(lead)
          if (!leadId) {
            spinner?.stop()
            throw new NotFoundError("Lead", lead)
          }
          input.leadId = leadId
        }

        if (teams != null) {
          input.teamIds = (await resolveTeams(teams)).map((t) => t.id)
        } else if (addTeam != null || removeTeam != null) {
          const toRef = (t: { id: string; key: string }): ResolvedRef => ({
            id: t.id,
            label: t.key,
          })
          const added = (await resolveTeams(addTeam ?? [])).map(toRef)
          const removed = (await resolveTeams(removeTeam ?? [])).map(toRef)
          rejectAddRemoveOverlap("team", added, removed)
          const current = await fetchAllPages(async (after) => {
            const data = await client.request(GetProjectTeamsForUpdate, {
              id: resolvedId,
              after,
            })
            return data.project.teams
          })
          const teamIds = applyCollectionEdit({
            current: current.map((t) => t.id),
            add: added,
            remove: removed,
            onMissing: (ref) =>
              new ValidationError(
                `Cannot remove team "${ref.label}": it is not on this project`,
                {
                  suggestion: `Current teams: ${
                    current.map((t) => `${t.key} (${t.name})`).join(", ")
                  }. Use --add-team to add one.`,
                },
              ),
          })
          // Verified against the API: projectUpdate rejects teamIds: [] with
          // "teamIds must contain at least 1 elements".
          if (teamIds.length === 0) {
            throw new ValidationError(
              "Removing these teams would leave the project with no teams; Linear requires at least one",
              {
                suggestion:
                  "Keep at least one team, or use --team to replace the set.",
              },
            )
          }
          input.teamIds = teamIds
        }

        if (labels != null) {
          // Replace the project's labels with exactly the resolved set,
          // matching `project update --team` and `issue update --label`.
          input.labelIds = (await resolveProjectLabels(labels)).map((l) => l.id)
        } else if (addLabel != null || removeLabel != null) {
          const added = await resolveProjectLabels(addLabel ?? [])
          const removed = await resolveProjectLabels(removeLabel ?? [])
          rejectAddRemoveOverlap("label", added, removed)
          const current = await fetchAllPages(async (after) => {
            const data = await client.request(GetProjectLabelsForUpdate, {
              id: resolvedId,
              after,
            })
            return data.project.labels
          })
          input.labelIds = applyCollectionEdit({
            current: current.map((l) => l.id),
            add: added,
            remove: removed,
            onMissing: (ref) =>
              new ValidationError(
                `Cannot remove label "${ref.label}": it is not on this project`,
                {
                  suggestion: current.length > 0
                    ? `Current labels: ${
                      current.map((l) => l.name).join(", ")
                    }. Use --add-label to add one.`
                    : "The project has no labels. Use --add-label to add one.",
                },
              ),
          })
        }

        let initiativeChanges: InitiativeChange[] = []
        let projectFromLinks: { name: string; url: string } | undefined
        if (
          initiatives != null || addInitiative != null ||
          removeInitiative != null
        ) {
          const replacement = initiatives != null
            ? await resolveInitiatives(client, initiatives)
            : undefined
          const added = await resolveInitiatives(client, addInitiative ?? [])
          const removed = await resolveInitiatives(
            client,
            removeInitiative ?? [],
          )
          rejectAddRemoveOverlap("initiative", added, removed)

          const links = await fetchAllPages(async (after) => {
            const data = await client.request(
              GetProjectInitiativeLinksForUpdate,
              { id: resolvedId, after },
            )
            projectFromLinks = {
              name: data.project.name,
              url: data.project.url,
            }
            return data.project.initiativeToProjects
          })
          const currentIds = links.map((l) => l.initiative.id)
          const desiredIds = replacement != null
            ? replacement.map((r) => r.id)
            : applyCollectionEdit({
              current: currentIds,
              add: added,
              remove: removed,
              onMissing: (ref) =>
                new ValidationError(
                  `Cannot remove initiative "${ref.label}": it is not linked to this project`,
                  {
                    suggestion: links.length > 0
                      ? `Current initiatives: ${
                        links.map((l) => l.initiative.name).join(", ")
                      }. Use --add-initiative to link one.`
                      : "The project is not linked to any initiative. Use --add-initiative to link one.",
                  },
                ),
            })
          const desired = new Set(desiredIds)
          const current = new Set(currentIds)
          const labelFor = (id: string): string =>
            [...replacement ?? [], ...added].find((r) => r.id === id)?.label ??
              id
          // Deletes first: a project may appear only once in an initiative
          // hierarchy, so moving it from an initiative to that initiative's
          // parent or child is rejected while the old link still exists.
          initiativeChanges = [
            ...links.filter((l) => !desired.has(l.initiative.id)).map((l) => ({
              kind: "remove" as const,
              linkId: l.id,
              initiativeId: l.initiative.id,
              label: l.initiative.name,
            })),
            ...desiredIds.filter((id) => !current.has(id)).map((id) => ({
              kind: "add" as const,
              initiativeId: id,
              label: labelFor(id),
            })),
          ]
        }

        // Everything above is resolution and validation; nothing has been
        // sent yet. An initiative-only update has nothing for projectUpdate.
        let project: { name: string; url: string } | undefined
        if (Object.keys(input).length > 0) {
          const result = await client.request(UpdateProject, {
            id: resolvedId,
            input,
          })
          if (!result.projectUpdate.success) {
            throw new CliError("Failed to update project")
          }
          const updated = result.projectUpdate.project
          if (updated) project = { name: updated.name, url: updated.url }
        } else {
          project = projectFromLinks
        }

        await applyInitiativeChanges(
          client,
          resolvedId,
          initiativeChanges,
          Object.keys(input).length > 0
            ? "updated the project's other fields"
            : undefined,
        )
        spinner?.stop()

        if (project) {
          console.log(`✓ Updated project: ${project.name}`)
          if (project.url) {
            console.log(project.url)
          }
        }
      } catch (error) {
        spinner?.stop()
        handleError(error, "Failed to update project")
      }
    },
  )
