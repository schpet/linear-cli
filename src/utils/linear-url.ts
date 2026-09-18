import { getCliWorkspace, getOption } from "../config.ts"
import { getDefaultWorkspace } from "../credentials.ts"
import { normalizeIssueIdentifier } from "./issue-identifier.ts"
import { ValidationError } from "./errors.ts"

/** Linear serves its app from this host only; it offers no custom domains. */
const LINEAR_APP_HOSTS: readonly string[] = ["linear.app", "www.linear.app"]

/**
 * Project, document and initiative URLs all end in `{name-slug}-{slugId}`,
 * where the slug ID is twelve hex characters. The name part is decorative.
 */
const SLUG_ID_RE = /^[0-9a-f]{12}$/

/** Comment and project-update anchors carry the first eight characters of a UUID. */
const COMMENT_ANCHOR_RE = /^comment-([0-9a-f]{8})$/
const PROJECT_UPDATE_ANCHOR_RE = /^project-update-[0-9a-f]{8}$/

/**
 * Team pages that still mean "this team". Linear puts other entities under
 * `/team/{KEY}/` too — a cycle lives at `/team/{KEY}/cycle/5` — so an unknown
 * descendant is refused rather than assumed to denote the team; `team delete`
 * reads this.
 *
 * `overview` (the team's home), `all`, `active`, `cycles`, `projects/all` and
 * `views/issues` are the pages the app's sidebar and tabs actually produce.
 * The rest are expected but unobserved; an entry for a page that does not
 * exist is harmless, since it can only fail to match.
 */
const TEAM_SUBPAGES: readonly string[] = [
  "overview",
  "all",
  "active",
  "backlog",
  "triage",
  "cycles",
  "projects",
  "projects/all",
  "views/issues",
  "settings",
]

/** Pages under a project that still mean the project. */
const PROJECT_SUBPAGES: readonly string[] = [
  "overview",
  "issues",
  "updates",
  "activity",
]

/**
 * The app addresses a cycle by its number, or relatively. Its `upcoming` is
 * the cycle the CLI already calls `next`. No other relative form was observed,
 * so no other is accepted.
 */
const CYCLE_ALIASES: Record<string, "active" | "next"> = {
  active: "active",
  upcoming: "next",
}

export type LinearUrlRef =
  | {
    kind: "issue"
    workspace: string
    identifier: string
    /**
     * Set when the URL pointed at a specific comment. Only the first eight
     * characters of the comment's UUID survive in the anchor, so this is
     * enough to explain the problem to the user but not to identify a comment.
     */
    commentIdPrefix?: string
  }
  | { kind: "project"; workspace: string; slugId: string }
  | { kind: "document"; workspace: string; slugId: string }
  | { kind: "initiative"; workspace: string; slugId: string }
  | { kind: "team"; workspace: string; teamKey: string }
  | {
    kind: "cycle"
    workspace: string
    teamKey: string
    /** A cycle number, or the relative cycle the URL names. */
    cycle: number | "active" | "next"
  }

export type LinearUrlParse =
  /** Ordinary input. Callers fall through to their existing lookup unchanged. */
  | { status: "not-linear-url" }
  /**
   * Recognisably a Linear URL, but not one that names something the CLI can
   * look up. Callers must report this rather than retrying it as a name — a
   * settings URL looked up as a project name is exactly the confusing
   * "not found: https://…" this module exists to remove.
   */
  | { status: "unsupported"; reason: string }
  | { status: "ok"; ref: LinearUrlRef }

const ENTITY_LABELS: Record<LinearUrlRef["kind"], string> = {
  issue: "an issue",
  project: "a project",
  document: "a document",
  initiative: "an initiative",
  team: "a team",
  cycle: "a cycle",
}

function unsupported(reason: string): LinearUrlParse {
  return { status: "unsupported", reason }
}

/**
 * Pull the twelve-hex slug ID off a `{name-slug}-{slugId}` path segment.
 *
 * A project can be named so that its slug is the whole segment, so a segment
 * that is itself a bare slug ID is accepted as-is. Anything else fails loudly
 * instead of sending a decorative slug to the API.
 */
function extractSlugId(segment: string): string | undefined {
  const lower = segment.toLowerCase()
  if (SLUG_ID_RE.test(lower)) {
    return lower
  }
  const lastDash = lower.lastIndexOf("-")
  if (lastDash === -1) {
    return undefined
  }
  const candidate = lower.slice(lastDash + 1)
  return SLUG_ID_RE.test(candidate) ? candidate : undefined
}

function toUrl(value: string): URL | undefined {
  const trimmed = value.trim()
  if (trimmed === "") {
    return undefined
  }

  // People paste `linear.app/...` without a scheme. Only add one for a host we
  // would accept anyway, so an ordinary name is never mistaken for a URL.
  const withScheme = /^https?:\/\//i.test(trimmed)
    ? trimmed
    : LINEAR_APP_HOSTS.some((host) =>
        trimmed.toLowerCase().startsWith(`${host}/`)
      )
    ? `https://${trimmed}`
    : trimmed

  try {
    return new URL(withScheme)
  } catch {
    return undefined
  }
}

function parseCyclePath(
  workspace: string,
  teamKey: string,
  rest: readonly string[],
): LinearUrlParse {
  const [segment, ...extra] = rest
  if (segment == null) {
    return unsupported("it does not name a cycle")
  }
  if (extra.length > 0) {
    return unsupported(`"${rest.join("/")}" is not a cycle page`)
  }

  const alias = CYCLE_ALIASES[segment.toLowerCase()]
  if (alias != null) {
    return {
      status: "ok",
      ref: {
        kind: "cycle",
        workspace,
        teamKey: teamKey.toUpperCase(),
        cycle: alias,
      },
    }
  }

  if (/^[1-9][0-9]*$/.test(segment)) {
    const cycle = Number(segment)
    if (Number.isSafeInteger(cycle)) {
      return {
        status: "ok",
        ref: {
          kind: "cycle",
          workspace,
          teamKey: teamKey.toUpperCase(),
          cycle,
        },
      }
    }
  }

  return unsupported(`"${segment}" is not a cycle number`)
}

/**
 * Classify a user-supplied reference.
 *
 * The URL is only ever read, never fetched, so `http` is accepted alongside
 * `https`. A port, embedded credentials, a subdomain or a lookalike host
 * (`linear.app.example.com`) all mean this is not a Linear URL.
 */
export function parseLinearUrl(value: string): LinearUrlParse {
  const url = toUrl(value)
  if (url == null) {
    return { status: "not-linear-url" }
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    return { status: "not-linear-url" }
  }
  const hostname = url.hostname.toLowerCase().replace(/\.$/, "")
  if (!LINEAR_APP_HOSTS.includes(hostname)) {
    return { status: "not-linear-url" }
  }
  if (url.port !== "" || url.username !== "" || url.password !== "") {
    return { status: "not-linear-url" }
  }

  let segments: string[]
  try {
    segments = url.pathname
      .split("/")
      .filter((segment) => segment !== "")
      .map((segment) => decodeURIComponent(segment))
  } catch {
    return unsupported("its path could not be decoded")
  }
  if (segments.some((segment) => segment === "." || segment === "..")) {
    return unsupported("its path contains relative segments")
  }

  const [workspace, entity, ...rest] = segments
  if (workspace == null || entity == null) {
    return unsupported("it does not name a workspace and an entity")
  }

  const anchor = url.hash.startsWith("#") ? url.hash.slice(1) : ""

  switch (entity) {
    case "issue": {
      const raw = rest[0]
      if (raw == null) {
        return unsupported("it does not name an issue")
      }
      const identifier = normalizeIssueIdentifier(raw)
      if (identifier == null) {
        return unsupported(`"${raw}" is not an issue identifier`)
      }
      if (anchor === "") {
        return { status: "ok", ref: { kind: "issue", workspace, identifier } }
      }
      const comment = anchor.match(COMMENT_ANCHOR_RE)
      if (comment?.[1] != null) {
        return {
          status: "ok",
          ref: {
            kind: "issue",
            workspace,
            identifier,
            commentIdPrefix: comment[1],
          },
        }
      }
      // An anchor we cannot read might mean something we would be ignoring.
      return unsupported(`"#${anchor}" is not a comment link`)
    }

    case "project":
    case "document":
    case "initiative": {
      const raw = rest[0]
      if (raw == null) {
        return unsupported(`it does not name ${ENTITY_LABELS[entity]}`)
      }
      const slugId = extractSlugId(raw)
      if (slugId == null) {
        return unsupported(`"${raw}" does not end in a Linear slug ID`)
      }
      const tail = rest.slice(1).join("/")
      // A project's tabs all mean the project: its overview, issues, updates,
      // and `/activity`, where status updates live (the update anchor names one
      // update, which nothing in the CLI addresses by ID). Anything else under
      // a project is refused rather than assumed to be the project.
      if (
        tail !== "" &&
        !(entity === "project" && PROJECT_SUBPAGES.includes(tail))
      ) {
        return unsupported(`"${tail}" is not a page this command can use`)
      }
      if (
        anchor !== "" && !PROJECT_UPDATE_ANCHOR_RE.test(anchor)
      ) {
        return unsupported(`"#${anchor}" is not a link this command can use`)
      }
      return { status: "ok", ref: { kind: entity, workspace, slugId } }
    }

    case "team": {
      const teamKey = rest[0]
      if (teamKey == null) {
        return unsupported("it does not name a team")
      }
      const tail = rest.slice(1).join("/")
      if (rest[1] === "cycle") {
        return parseCyclePath(workspace, teamKey, rest.slice(2))
      }
      if (tail !== "" && !TEAM_SUBPAGES.includes(tail)) {
        return unsupported(`"${tail}" is not a team page this command can use`)
      }
      return {
        status: "ok",
        ref: { kind: "team", workspace, teamKey: teamKey.toUpperCase() },
      }
    }

    default:
      return unsupported(`"${entity}" is not an entity this command can use`)
  }
}

/**
 * The workspace this invocation is working in, as far as can be told without a
 * request: the `--workspace` flag, then the configured workspace, then the
 * default credential.
 *
 * Under a raw `LINEAR_API_KEY` the key's organization is not knowable locally,
 * so the configured slug is a declaration rather than proof. Trusting it anyway
 * is deliberate and matches what the CLI already does elsewhere —
 * `openProjectPage` builds the URLs a user opens out of this same option. The
 * alternative, staying silent whenever a raw key is in play, would disable the
 * check for the most common setup, which is exactly when someone pastes a URL
 * from the wrong workspace. A false positive costs one clear, self-explanatory
 * error naming both workspaces.
 */
function getEffectiveWorkspaceSlug(): string | undefined {
  const configured = getCliWorkspace() ?? getOption("workspace") ??
    getDefaultWorkspace()
  const trimmed = configured?.trim()
  return trimmed == null || trimmed === "" ? undefined : trimmed
}

function assertSameWorkspace(urlWorkspace: string): void {
  const current = getEffectiveWorkspaceSlug()
  if (current == null) {
    return
  }
  if (current.toLowerCase() === urlWorkspace.toLowerCase()) {
    return
  }
  throw new ValidationError(
    `That URL is for the "${urlWorkspace}" workspace, but this is the "${current}" workspace.`,
    { suggestion: switchWorkspaceSuggestion(urlWorkspace, current) },
  )
}

/**
 * How to reach another workspace depends on where the API key comes from,
 * mirroring the precedence in `getResolvedApiKey`. Suggesting --workspace to
 * someone whose key comes from LINEAR_API_KEY would send them straight into a
 * second error — the CLI refuses that combination — and a config `api_key`
 * outranks --workspace, so there the flag would silently change nothing.
 */
function switchWorkspaceSuggestion(
  urlWorkspace: string,
  current: string,
): string {
  const fromUrl = `or use a URL from "${current}".`
  if (Deno.env.get("LINEAR_API_KEY") != null) {
    return `LINEAR_API_KEY is set, and the CLI won't combine it with --workspace. Unset it and pass --workspace ${urlWorkspace}, ${fromUrl}`
  }
  if (getOption("api_key") != null) {
    return `The api_key in your config outranks --workspace. Remove it to pass --workspace ${urlWorkspace}, ${fromUrl}`
  }
  return `Pass --workspace ${urlWorkspace}, ${fromUrl}`
}

/**
 * Extract a reference of the expected kind from a URL.
 *
 * Returns `undefined` when the input is not a Linear URL at all, so callers keep
 * their existing UUID / slug / name handling for ordinary input. Throws when the
 * input is a Linear URL that names something else, names nothing usable, or
 * belongs to another workspace.
 */
/**
 * TypeScript cannot narrow a union by comparing against a generic kind, so the
 * relationship is stated once here as a type predicate rather than asserted
 * with a cast at the call site.
 */
function isRefOfKind<K extends LinearUrlRef["kind"]>(
  ref: LinearUrlRef,
  kind: K,
): ref is Extract<LinearUrlRef, { kind: K }> {
  return ref.kind === kind
}

export function expectLinearUrlKind<K extends LinearUrlRef["kind"]>(
  input: string,
  kind: K,
  entityLabel: string,
): Extract<LinearUrlRef, { kind: K }> | undefined {
  const parsed = parseLinearUrl(input)
  if (parsed.status === "not-linear-url") {
    return undefined
  }
  if (parsed.status === "unsupported") {
    throw new ValidationError(
      `"${input}" is a Linear URL, but ${parsed.reason}.`,
      { suggestion: `Pass ${entityLabel}.` },
    )
  }

  const { ref } = parsed
  assertSameWorkspace(ref.workspace)

  if (!isRefOfKind(ref, kind)) {
    throw new ValidationError(
      `"${input}" is ${ENTITY_LABELS[ref.kind]} URL, not ${
        ENTITY_LABELS[kind]
      } URL.`,
      { suggestion: `Pass ${entityLabel}.` },
    )
  }

  return ref
}

/**
 * For commands whose identifiers have no Linear URL at all (milestones, labels,
 * templates, releases, agent sessions). A pasted URL is reported plainly
 * instead of becoming a lookup for a string starting with `https://`.
 */
export function rejectLinearUrl(input: string, entityLabel: string): void {
  const parsed = parseLinearUrl(input)
  if (parsed.status === "not-linear-url") {
    return
  }
  throw new ValidationError(
    `"${input}" is a Linear URL, and this command does not take one.`,
    { suggestion: `Pass ${entityLabel}.` },
  )
}

/**
 * A comment URL keeps only the first eight characters of the comment's UUID, so
 * it cannot identify a comment. Commands that act on a comment say so directly
 * rather than looking up something they cannot pin down.
 */
export function rejectCommentUrl(input: string): void {
  const parsed = parseLinearUrl(input)
  if (parsed.status !== "ok") {
    return
  }
  if (parsed.ref.kind === "issue" && parsed.ref.commentIdPrefix != null) {
    throw new ValidationError(
      `"${input}" links to a comment, but a comment URL only carries the first eight characters of its ID.`,
      {
        suggestion:
          "Pass the comment's full UUID, from `linear issue comment list <issue> --json`.",
      },
    )
  }
}
