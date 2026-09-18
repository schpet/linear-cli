/**
 * Property-based fuzzing for the Linear URL parser.
 *
 * `parseLinearUrl` is a pure function from an arbitrary user-pasted string to a
 * typed classification, which makes it the kind of narrow, deterministic
 * boundary where generative testing pays off. Each property below is an oracle
 * the parser has to keep for every input, not just the handful in
 * `linear-url.test.ts`:
 *
 * - round-trip: any reference, rendered the way people paste it, parses back
 * - containment: only `linear.app` itself is ever read as Linear
 * - fall-through: ordinary IDs, slugs and names are never mistaken for URLs
 * - totality: junk and mutated URLs never throw, and whatever comes out is
 *   well-formed
 * - kind checking: the right kind returns the reference, any other kind is a
 *   ValidationError, and nothing else ever escapes
 *
 * Each run uses a fresh random seed, so CI keeps exploring new inputs. A
 * failure prints the seed and a shrunk counterexample. To replay one exactly:
 *
 *   FC_SEED=<seed> FC_PATH=<path> deno task test test/utils/linear-url.property.test.ts
 *
 * For a longer campaign than CI runs, raise FC_NUM_RUNS (e.g. 200000). Any
 * failure found that way should also be added to linear-url.test.ts as a
 * plain regression case.
 */
import fc from "fast-check"
import { assertEquals, assertThrows } from "@std/assert"
import {
  expectLinearUrlKind,
  type LinearUrlParse,
  type LinearUrlRef,
  parseLinearUrl,
} from "../../src/utils/linear-url.ts"
import { ValidationError } from "../../src/utils/errors.ts"

function runParameters(): fc.Parameters<unknown> {
  const seed = Deno.env.get("FC_SEED")
  const path = Deno.env.get("FC_PATH")
  const numRuns = Deno.env.get("FC_NUM_RUNS")
  return {
    numRuns: numRuns == null ? 1000 : Number(numRuns),
    ...(seed == null ? {} : { seed: Number(seed) }),
    ...(path == null ? {} : { path }),
  }
}

const KINDS: ReadonlyArray<LinearUrlRef["kind"]> = [
  "issue",
  "project",
  "document",
  "initiative",
  "team",
  "cycle",
]

// ---------------------------------------------------------------------------
// Generators for the pieces a real Linear URL is made of.

const workspaceArb = fc.stringMatching(/^[a-z0-9][a-z0-9-]{0,19}$/)
const teamKeyArb = fc.stringMatching(/^[A-Z][A-Z0-9]{0,6}$/)
const slugIdArb = fc.stringMatching(/^[0-9a-f]{12}$/)
const shortHexArb = fc.stringMatching(/^[0-9a-f]{8}$/)
/**
 * The decorative part of `{name-slug}-{slugId}`. It is deliberately allowed to
 * look like hex, run to extra dashes, or be empty — the slug ID must still be
 * found at the end.
 */
const nameSlugArb = fc.stringMatching(/^[a-z0-9-]{0,40}$/)
const titleSlugArb = fc.option(fc.stringMatching(/^[a-z0-9-]{1,40}$/), {
  nil: undefined,
})

function withSlug(nameSlug: string, slugId: string): string {
  return nameSlug === "" ? slugId : `${nameSlug}-${slugId}`
}

/** A URL path (after the workspace) and the reference it should parse to. */
type Case = { path: string; anchor: string; expected: LinearUrlRef }

function caseArb(workspace: string): fc.Arbitrary<Case> {
  const lowerKey = (key: string, lower: boolean) =>
    lower ? key.toLowerCase() : key

  const issue = fc.record({
    teamKey: teamKeyArb,
    number: fc.integer({ min: 1, max: 1_000_000 }),
    title: titleSlugArb,
    comment: fc.option(shortHexArb, { nil: undefined }),
    lower: fc.boolean(),
  }).map(({ teamKey, number, title, comment, lower }): Case => ({
    path: `issue/${lowerKey(teamKey, lower)}-${number}${
      title == null ? "" : `/${title}`
    }`,
    anchor: comment == null ? "" : `#comment-${comment}`,
    expected: {
      kind: "issue",
      workspace,
      identifier: `${teamKey}-${number}`,
      ...(comment == null ? {} : { commentIdPrefix: comment }),
    },
  }))

  const project = fc.record({
    name: nameSlugArb,
    slugId: slugIdArb,
    tab: fc.constantFrom("", "overview", "issues", "updates", "activity"),
    update: fc.option(shortHexArb, { nil: undefined }),
  }).map(({ name, slugId, tab, update }): Case => ({
    path: `project/${withSlug(name, slugId)}${tab === "" ? "" : `/${tab}`}`,
    anchor: update == null ? "" : `#project-update-${update}`,
    expected: { kind: "project", workspace, slugId },
  }))

  const documentOrInitiative = fc.record({
    kind: fc.constantFrom<"document" | "initiative">("document", "initiative"),
    name: nameSlugArb,
    slugId: slugIdArb,
  }).map(({ kind, name, slugId }): Case => ({
    path: `${kind}/${withSlug(name, slugId)}`,
    anchor: "",
    expected: { kind, workspace, slugId },
  }))

  const team = fc.record({
    teamKey: teamKeyArb,
    page: fc.constantFrom(
      "",
      "overview",
      "all",
      "active",
      "cycles",
      "projects/all",
      "views/issues",
    ),
    lower: fc.boolean(),
  }).map(({ teamKey, page, lower }): Case => ({
    path: `team/${lowerKey(teamKey, lower)}${page === "" ? "" : `/${page}`}`,
    anchor: "",
    expected: { kind: "team", workspace, teamKey },
  }))

  const cycle = fc.record({
    teamKey: teamKeyArb,
    which: fc.oneof(
      fc.integer({ min: 1, max: 100_000 }),
      fc.constantFrom<"active" | "upcoming">("active", "upcoming"),
    ),
  }).map(({ teamKey, which }): Case => ({
    path: `team/${teamKey}/cycle/${which}`,
    anchor: "",
    expected: {
      kind: "cycle",
      workspace,
      teamKey,
      // The app's "upcoming" is the cycle the CLI calls "next".
      cycle: which === "upcoming" ? "next" : which,
    },
  }))

  return fc.oneof(issue, project, documentOrInitiative, team, cycle)
}

/** The ways the same URL arrives from a browser, the app, or a sloppy paste. */
const presentationArb = fc.record({
  scheme: fc.constantFrom("https://", "http://", ""),
  host: fc.constantFrom(
    "linear.app",
    "www.linear.app",
    "LINEAR.APP",
    "Linear.App",
  ),
  trailingSlash: fc.boolean(),
  query: fc.constantFrom("", "?utm_source=slack", "?a=1&b=2", "?"),
  padding: fc.constantFrom("", " ", "  ", "\t", "\n"),
})

type Rendered = { url: string; expected: LinearUrlRef }

const renderedArb: fc.Arbitrary<Rendered> = workspaceArb.chain((workspace) =>
  fc.tuple(caseArb(workspace), presentationArb).map(
    ([{ path, anchor, expected }, p]) => ({
      url: `${p.padding}${p.scheme}${p.host}/${workspace}/${path}${
        p.trailingSlash ? "/" : ""
      }${p.query}${anchor}${p.padding}`,
      expected,
    }),
  )
)

// ---------------------------------------------------------------------------
// Properties.

Deno.test("property: every reference survives the ways people paste its URL", () => {
  fc.assert(
    fc.property(renderedArb, ({ url, expected }) => {
      assertEquals(parseLinearUrl(url), { status: "ok", ref: expected })
    }),
    runParameters(),
  )
})

/**
 * The WHATWG URL parser is the reference: whatever host it sees, only the two
 * real Linear hosts may be treated as Linear. This is the check that stops a
 * lookalike domain from being read as a reference into the workspace.
 */
Deno.test("property: only linear.app itself is ever read as a Linear URL", () => {
  const lookalikeHostArb = fc.tuple(
    fc.constantFrom("", "www.", "app.", "sub.", "linear-", "x"),
    fc.constantFrom("linear.app", "linear-app", "linearapp", "lineaR.app"),
    fc.constantFrom("", ".", ".evil.com", ".app", "x", ":8443", "-cdn"),
  ).map(([before, core, after]) => `${before}${core}${after}`)

  const hostArb = fc.oneof(lookalikeHostArb, fc.domain())

  fc.assert(
    fc.property(
      hostArb,
      fc.constantFrom("https://", "http://"),
      fc.constantFrom(
        "/acme/issue/ENG-1/x",
        "/acme/project/a-576342554a6e",
        "/acme/team/ENG/cycle/5",
      ),
      (host, scheme, path) => {
        const input = `${scheme}${host}${path}`
        let reference: URL
        try {
          reference = new URL(input)
        } catch {
          // Not a URL at all, so it must fall through as ordinary input.
          assertEquals(parseLinearUrl(input).status, "not-linear-url", input)
          return
        }
        const real = ["linear.app", "www.linear.app"].includes(
          reference.hostname.toLowerCase().replace(/\.$/, ""),
        ) && reference.port === ""
        if (!real) {
          assertEquals(parseLinearUrl(input).status, "not-linear-url", input)
        }
      },
    ),
    runParameters(),
  )
})

/**
 * Every existing lookup — IDs, slug IDs, UUIDs, names — depends on ordinary
 * input falling through untouched. Only input that plainly announces itself as
 * a URL may be classified as one.
 */
Deno.test("property: ordinary identifiers and names are never mistaken for URLs", () => {
  const nameArb = fc.string({ unit: "grapheme", maxLength: 60 }).filter(
    (value) => {
      const lead = value.trim().toLowerCase()
      return !["http:", "https:", "linear.app/", "www.linear.app/"].some(
        (prefix) => lead.startsWith(prefix),
      )
    },
  )
  const ordinaryArb = fc.oneof(
    fc.tuple(teamKeyArb, fc.integer({ min: 1, max: 1_000_000 })).map((
      [key, n],
    ) => `${key}-${n}`),
    slugIdArb,
    fc.uuid(),
    nameArb,
  )

  fc.assert(
    fc.property(ordinaryArb, (value) => {
      assertEquals(parseLinearUrl(value), { status: "not-linear-url" }, value)
    }),
    runParameters(),
  )
})

/** Garbage in must still produce a well-formed classification, never a throw. */
function assertWellFormed(parse: LinearUrlParse, input: string): void {
  if (parse.status !== "ok") {
    return
  }
  const { ref } = parse
  switch (ref.kind) {
    case "issue":
      assertEquals(/^[A-Z0-9]+-[1-9][0-9]*$/.test(ref.identifier), true, input)
      if (ref.commentIdPrefix != null) {
        assertEquals(/^[0-9a-f]{8}$/.test(ref.commentIdPrefix), true, input)
      }
      return
    case "project":
    case "document":
    case "initiative":
      assertEquals(/^[0-9a-f]{12}$/.test(ref.slugId), true, input)
      return
    case "team":
      assertEquals(ref.teamKey, ref.teamKey.toUpperCase(), input)
      return
    case "cycle":
      assertEquals(ref.teamKey, ref.teamKey.toUpperCase(), input)
      if (typeof ref.cycle === "number") {
        assertEquals(
          Number.isSafeInteger(ref.cycle) && ref.cycle > 0,
          true,
          input,
        )
      }
      return
    default: {
      const unreachable: never = ref
      throw new Error(`unhandled kind in ${JSON.stringify(unreachable)}`)
    }
  }
}

/**
 * Start from a real, valid URL and damage it: insert, delete and replace
 * characters anywhere. This reaches the parser's inner branches far more often
 * than random strings, which almost never get past the host check.
 */
const mutatedArb: fc.Arbitrary<string> = fc.tuple(
  renderedArb,
  fc.array(
    fc.record({
      at: fc.nat(),
      op: fc.constantFrom("insert", "delete", "replace"),
      char: fc.constantFrom(
        "/",
        "-",
        "#",
        "?",
        "%",
        ".",
        ":",
        "@",
        " ",
        "a",
        "Z",
        "0",
        "9",
        "é",
        "\\",
        "%2F",
        "%E0%A4%A",
        "..",
      ),
    }),
    { maxLength: 6 },
  ),
).map(([{ url }, edits]) =>
  edits.reduce((current, { at, op, char }) => {
    const i = current.length === 0 ? 0 : at % (current.length + 1)
    switch (op) {
      case "insert":
        return current.slice(0, i) + char + current.slice(i)
      case "delete":
        return current.slice(0, i) + current.slice(i + 1)
      case "replace":
        return current.slice(0, i) + char + current.slice(i + 1)
    }
  }, url)
)

Deno.test("property: junk and damaged URLs never throw, and results are well-formed", () => {
  const inputArb = fc.oneof(
    mutatedArb,
    fc.string({ unit: "binary", maxLength: 120 }),
    fc.webUrl({ withQueryParameters: true, withFragments: true }),
  )
  fc.assert(
    fc.property(inputArb, (input) => {
      const parse = parseLinearUrl(input)
      assertEquals(
        ["not-linear-url", "unsupported", "ok"].includes(parse.status),
        true,
      )
      assertWellFormed(parse, input)
    }),
    runParameters(),
  )
})

function withWorkspace<T>(workspace: string, fn: () => T): T {
  const previous = Deno.env.get("LINEAR_WORKSPACE")
  Deno.env.set("LINEAR_WORKSPACE", workspace)
  try {
    return fn()
  } finally {
    if (previous == null) {
      Deno.env.delete("LINEAR_WORKSPACE")
    } else {
      Deno.env.set("LINEAR_WORKSPACE", previous)
    }
  }
}

Deno.test("property: the right kind returns the reference and every other kind is refused", () => {
  fc.assert(
    fc.property(renderedArb, ({ url, expected }) => {
      withWorkspace(expected.workspace, () => {
        assertEquals(
          expectLinearUrlKind(url, expected.kind, "a reference"),
          expected,
        )
        for (const other of KINDS.filter((k) => k !== expected.kind)) {
          assertThrows(
            () => expectLinearUrlKind(url, other, "a reference"),
            ValidationError,
          )
        }
      })
    }),
    runParameters(),
  )
})

/**
 * Whatever a user pastes into any slot, the only failure that may escape is a
 * ValidationError — which handleError turns into a clean message. A TypeError
 * or URIError would surface as a stack trace instead.
 */
Deno.test("property: pasted junk only ever fails as a ValidationError", () => {
  const inputArb = fc.oneof(
    mutatedArb,
    fc.string({ unit: "binary", maxLength: 120 }),
  )
  fc.assert(
    fc.property(inputArb, fc.constantFrom(...KINDS), (input, kind) => {
      withWorkspace("url-test-workspace", () => {
        try {
          expectLinearUrlKind(input, kind, "a reference")
        } catch (error) {
          assertEquals(error instanceof ValidationError, true, String(error))
        }
      })
    }),
    runParameters(),
  )
})
