import { assertEquals, assertThrows } from "@std/assert"
import {
  expectLinearUrlKind,
  type LinearUrlParse,
  parseLinearUrl,
  rejectCommentUrl,
  rejectLinearUrl,
} from "../../src/utils/linear-url.ts"
import { ValidationError } from "../../src/utils/errors.ts"

/**
 * The canonical shapes, copied from the `url` field Linear's own API returns.
 * If Linear changes a shape, these are the cases that should break first.
 */
Deno.test("parseLinearUrl reads the URL shapes Linear itself produces", () => {
  const cases: Array<[string, LinearUrlParse]> = [
    [
      "https://linear.app/schpet/issue/CLI-113/cycle-demo-safe-to-delete",
      {
        status: "ok",
        ref: { kind: "issue", workspace: "schpet", identifier: "CLI-113" },
      },
    ],
    [
      "https://linear.app/schpet/project/triage-221-225-2026-05-22-576342554a6e",
      {
        status: "ok",
        ref: { kind: "project", workspace: "schpet", slugId: "576342554a6e" },
      },
    ],
    [
      "https://linear.app/schpet/document/repro-225-document-bb0b3827273a",
      {
        status: "ok",
        ref: { kind: "document", workspace: "schpet", slugId: "bb0b3827273a" },
      },
    ],
    [
      "https://linear.app/schpet/initiative/url-shape-probe-1e2f47b4f76d",
      {
        status: "ok",
        ref: {
          kind: "initiative",
          workspace: "schpet",
          slugId: "1e2f47b4f76d",
        },
      },
    ],
    [
      "https://linear.app/schpet/issue/CLI-72/release-script#comment-325482e4",
      {
        status: "ok",
        ref: {
          kind: "issue",
          workspace: "schpet",
          identifier: "CLI-72",
          commentIdPrefix: "325482e4",
        },
      },
    ],
    [
      // A project-update link is the project as far as the CLI is concerned.
      "https://linear.app/schpet/project/qa-test-project-b7112e0dcc53/activity#project-update-f0896970",
      {
        status: "ok",
        ref: { kind: "project", workspace: "schpet", slugId: "b7112e0dcc53" },
      },
    ],
  ]

  for (const [url, expected] of cases) {
    assertEquals(parseLinearUrl(url), expected, url)
  }
})

Deno.test("parseLinearUrl accepts the variations people actually paste", () => {
  const issue = {
    status: "ok",
    ref: { kind: "issue", workspace: "schpet", identifier: "CLI-113" },
  } satisfies LinearUrlParse

  // No scheme — copied from a browser's address bar.
  assertEquals(parseLinearUrl("linear.app/schpet/issue/CLI-113/x"), issue)
  // Surrounding whitespace from a sloppy paste.
  assertEquals(
    parseLinearUrl("  https://linear.app/schpet/issue/CLI-113/x  "),
    issue,
  )
  // Host casing and www.
  assertEquals(
    parseLinearUrl("https://LINEAR.APP/schpet/issue/CLI-113/x"),
    issue,
  )
  assertEquals(
    parseLinearUrl("https://www.linear.app/schpet/issue/CLI-113/x"),
    issue,
  )
  // Tracking parameters never affect identity.
  assertEquals(
    parseLinearUrl("https://linear.app/schpet/issue/CLI-113/x?foo=bar"),
    issue,
  )
  // No title slug.
  assertEquals(parseLinearUrl("https://linear.app/schpet/issue/CLI-113"), issue)
  // A lowercase team key is normalised the way identifiers are everywhere else.
  assertEquals(parseLinearUrl("https://linear.app/schpet/issue/cli-113"), issue)
  // Read, never fetched, so plain http is fine.
  assertEquals(
    parseLinearUrl("http://linear.app/schpet/issue/CLI-113/x"),
    issue,
  )
})

Deno.test("parseLinearUrl leaves ordinary input alone", () => {
  // Anything here that came back "unsupported" would break an existing lookup
  // by turning a legitimate name into an error.
  const ordinary = [
    "CLI-113",
    "Mobile launch",
    "576342554a6e",
    "85d3dad6-136e-49ff-9593-33dc4b22b5ee",
    "",
    "   ",
    // A different host entirely — `issue link` depends on this staying inert.
    "https://github.com/schpet/linear-cli/pull/290",
    // Lookalike hosts must not be mistaken for Linear.
    "https://linear.app.evil.example/schpet/issue/CLI-1",
    "https://notlinear.app/schpet/issue/CLI-1",
    "https://sub.linear.app/schpet/issue/CLI-1",
    // A port or credentials mean this is not the real thing.
    "https://linear.app:8443/schpet/issue/CLI-1",
    "https://user:pw@linear.app/schpet/issue/CLI-1",
    // Not an http(s) scheme.
    "linear://schpet/issue/CLI-1",
    // A project could genuinely be named this.
    "some/name/with/slashes",
  ]

  for (const value of ordinary) {
    assertEquals(
      parseLinearUrl(value),
      { status: "not-linear-url" },
      value,
    )
  }
})

Deno.test("parseLinearUrl refuses Linear URLs it cannot turn into a reference", () => {
  // Each of these would otherwise be looked up as a name and reported as
  // "not found: https://…", which is the confusion this module removes.
  const unusable = [
    "https://linear.app/schpet/settings/members",
    "https://linear.app/schpet/search?q=hello",
    "https://linear.app/schpet",
    "https://linear.app/schpet/issue",
    "https://linear.app/schpet/issue/not-an-identifier/x",
    // The slug ID is the part that identifies the entity; without it there is
    // nothing to look up.
    "https://linear.app/schpet/project/no-slug-here",
    "https://linear.app/schpet/project/short-abc123",
    // A page under a project that is not the project.
    "https://linear.app/schpet/project/qa-test-project-b7112e0dcc53/settings",
    // An anchor we cannot read might mean something we would silently ignore.
    "https://linear.app/schpet/issue/CLI-1/x#activity",
    "https://linear.app/schpet/issue/CLI-1/x#comment-zzzz",
    "https://linear.app/schpet/team",
    "https://linear.app/schpet/team/CLI/unknown-page",
  ]

  for (const value of unusable) {
    assertEquals(parseLinearUrl(value).status, "unsupported", value)
  }
})

Deno.test("parseLinearUrl treats a project's tabs as the project", () => {
  // A project URL is usually copied while looking at one of its tabs.
  const project = {
    status: "ok",
    ref: { kind: "project", workspace: "schpet", slugId: "b7112e0dcc53" },
  } satisfies LinearUrlParse
  const base = "https://linear.app/schpet/project/qa-test-project-b7112e0dcc53"
  for (const tab of ["", "/overview", "/issues", "/updates", "/activity"]) {
    assertEquals(parseLinearUrl(base + tab), project, tab || "(bare)")
  }
  // Anything else under a project is not assumed to be the project.
  assertEquals(parseLinearUrl(base + "/settings").status, "unsupported")
})

Deno.test("parseLinearUrl treats a team's own pages as the team", () => {
  const team = {
    status: "ok",
    ref: { kind: "team", workspace: "schpet", teamKey: "CLI" },
  } satisfies LinearUrlParse

  for (
    const url of [
      "https://linear.app/schpet/team/CLI",
      // The team's home page, copied from the app's sidebar.
      "https://linear.app/schpet/team/CLI/overview",
      "https://linear.app/schpet/team/CLI/all",
      "https://linear.app/schpet/team/CLI/active",
      "https://linear.app/schpet/team/CLI/cycles",
      "https://linear.app/schpet/team/CLI/projects/all",
      "https://linear.app/schpet/team/CLI/views/issues",
      "https://linear.app/schpet/team/cli/settings",
    ]
  ) {
    assertEquals(parseLinearUrl(url), team, url)
  }
})

Deno.test("parseLinearUrl reads the cycle URLs the app produces", () => {
  // Each shape here was copied out of the Linear app: the app addresses a
  // cycle by number, or relatively as "active" or "upcoming". The CLI's own
  // word for "upcoming" is "next".
  const cycle = (value: number | "active" | "next"): LinearUrlParse => ({
    status: "ok",
    ref: { kind: "cycle", workspace: "schpet", teamKey: "CLI", cycle: value },
  })

  assertEquals(
    parseLinearUrl("https://linear.app/schpet/team/CLI/cycle/5"),
    cycle(5),
  )
  assertEquals(
    parseLinearUrl("https://linear.app/schpet/team/CLI/cycle/active"),
    cycle("active"),
  )
  assertEquals(
    parseLinearUrl("https://linear.app/schpet/team/CLI/cycle/upcoming"),
    cycle("next"),
  )
  // Team keys normalise the way they do everywhere else.
  assertEquals(
    parseLinearUrl("https://linear.app/schpet/team/cli/cycle/5"),
    cycle(5),
  )
})

Deno.test("parseLinearUrl refuses cycle URLs it has not seen the app produce", () => {
  // "previous" was not observed in the app, so it is not guessed at.
  for (
    const url of [
      "https://linear.app/schpet/team/CLI/cycle",
      "https://linear.app/schpet/team/CLI/cycle/previous",
      "https://linear.app/schpet/team/CLI/cycle/abc",
      "https://linear.app/schpet/team/CLI/cycle/0",
      "https://linear.app/schpet/team/CLI/cycle/5/issues",
    ]
  ) {
    assertEquals(parseLinearUrl(url).status, "unsupported", url)
  }
})

Deno.test("expectLinearUrlKind falls through for ordinary input", () => {
  // undefined is what preserves every existing UUID / slug / name lookup.
  assertEquals(expectLinearUrlKind("CLI-113", "issue", "an issue"), undefined)
  assertEquals(
    expectLinearUrlKind("Mobile launch", "project", "a project"),
    undefined,
  )
})

Deno.test("expectLinearUrlKind names what the URL actually points at", () => {
  const error = assertThrows(
    () =>
      expectLinearUrlKind(
        "https://linear.app/schpet/issue/CLI-113/x",
        "project",
        "a project URL, UUID, slug ID, or exact name",
      ),
    ValidationError,
    "is an issue URL, not a project URL",
  )
  // The suggestion tells the user what to pass instead; it is a separate field
  // from the message, and `handleError` prints it on its own line.
  assertEquals(
    error.suggestion,
    "Pass a project URL, UUID, slug ID, or exact name.",
  )
})

Deno.test("expectLinearUrlKind returns the extracted reference, never the URL", () => {
  const ref = expectLinearUrlKind(
    "https://linear.app/schpet/project/triage-221-225-2026-05-22-576342554a6e",
    "project",
    "a project",
  )
  // Sending the whole URL to GraphQL happens to work today through an
  // undocumented server-side behavior; this pins the local extraction instead.
  assertEquals(ref?.slugId, "576342554a6e")
})

Deno.test("rejectLinearUrl reports a URL a command cannot take", () => {
  assertThrows(
    () =>
      rejectLinearUrl(
        "https://linear.app/schpet/issue/CLI-113/x",
        "a milestone name or UUID",
      ),
    ValidationError,
    "does not take one",
  )
  // Ordinary input passes straight through.
  rejectLinearUrl("Design complete", "a milestone name or UUID")
})

Deno.test("rejectCommentUrl explains why a comment link cannot identify a comment", () => {
  assertThrows(
    () =>
      rejectCommentUrl(
        "https://linear.app/schpet/issue/CLI-72/x#comment-325482e4",
      ),
    ValidationError,
    "first eight characters",
  )
  // An issue URL without a comment anchor is somebody else's problem.
  rejectCommentUrl("https://linear.app/schpet/issue/CLI-72/x")
  rejectCommentUrl("325482e4-6d9f-4a97-8f6d-3fc5b4a1fb39")
})

/**
 * Run with LINEAR_WORKSPACE and LINEAR_API_KEY pinned, then restore them. The
 * suggestion depends on where the key comes from, so leaving either to the
 * machine running the test would make it pass in one place and fail in another.
 */
function withAuthEnv(
  env: { workspace: string; apiKey: string | undefined },
  fn: () => void,
): void {
  const previous = {
    workspace: Deno.env.get("LINEAR_WORKSPACE"),
    apiKey: Deno.env.get("LINEAR_API_KEY"),
  }
  const apply = (name: string, value: string | undefined) =>
    value == null ? Deno.env.delete(name) : Deno.env.set(name, value)
  apply("LINEAR_WORKSPACE", env.workspace)
  apply("LINEAR_API_KEY", env.apiKey)
  try {
    fn()
  } finally {
    apply("LINEAR_WORKSPACE", previous.workspace)
    apply("LINEAR_API_KEY", previous.apiKey)
  }
}

const FOREIGN_URL = "https://linear.app/acme/issue/ENG-1/x"

Deno.test("expectLinearUrlKind refuses a URL from another workspace", () => {
  // A value that cannot coincide with whatever workspace the machine running
  // this test happens to be configured for.
  withAuthEnv({ workspace: "url-test-workspace", apiKey: undefined }, () => {
    const error = assertThrows(
      () => expectLinearUrlKind(FOREIGN_URL, "issue", "an issue URL"),
      ValidationError,
      'That URL is for the "acme" workspace',
    )
    assertEquals(
      error.suggestion,
      'Pass --workspace acme, or use a URL from "url-test-workspace".',
    )

    // The same workspace, spelled differently, is still the same workspace.
    assertEquals(
      expectLinearUrlKind(
        "https://linear.app/URL-TEST-WORKSPACE/issue/CLI-1/x",
        "issue",
        "an issue URL",
      )?.identifier,
      "CLI-1",
    )
  })
})

Deno.test("a workspace mismatch under LINEAR_API_KEY does not suggest --workspace alone", () => {
  // The CLI refuses --workspace while LINEAR_API_KEY is set, so a bare
  // "pass --workspace" would lead straight into a second error.
  withAuthEnv(
    { workspace: "url-test-workspace", apiKey: "lin_api_test" },
    () => {
      const error = assertThrows(
        () => expectLinearUrlKind(FOREIGN_URL, "issue", "an issue URL"),
        ValidationError,
      )
      assertEquals(
        error.suggestion,
        'LINEAR_API_KEY is set, and the CLI won\'t combine it with --workspace. Unset it and pass --workspace acme, or use a URL from "url-test-workspace".',
      )
    },
  )
})
