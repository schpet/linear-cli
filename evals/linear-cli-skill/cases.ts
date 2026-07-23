/**
 * Frozen case manifest for the linear-cli skill eval.
 *
 * Five recipe families with one development + one holdout prompt each, plus
 * two controls where raw GraphQL genuinely is the right route. Prompts state
 * the workspace facts they need and are neutral about mechanism.
 *
 * Frozen before the baseline run — do not edit prompts, expectations, or add
 * harder cases after seeing baseline results.
 */

export interface RequiredArg {
  /** Accepted flag spellings, e.g. ["--state", "-s"] */
  flags: string[]
  /** If set, the flag's value must equal this (or end with it for paths) */
  value?: string
  /** Match value by suffix (for file paths whose prefix is the tmp workdir) */
  valueIsPathSuffix?: boolean
}

export interface RequiredPositional {
  /** Expected value, compared exactly (or by suffix for paths) */
  value: string
  /** Match by suffix (for file paths whose prefix is the tmp workdir) */
  valueIsPathSuffix?: boolean
}

export interface CliExpectation {
  route: "cli"
  /**
   * Subcommand prefixes that count as the dedicated-CLI route for this task
   * (e.g. any issue-listing command). First matching prefix wins.
   */
  routePrefixes: string[][]
  /**
   * Stricter args_ok tier: the exact subcommand prefix the recipes teach,
   * plus flags that must be present (order/alias insensitive).
   */
  argsPrefix: string[]
  requiredArgs: RequiredArg[]
  /**
   * Positional arguments that must appear after `argsPrefix`, in order.
   * Grading strips the flags declared in `valueFlags`/`booleanFlags` (plus
   * their values) from the tail of argv before comparing, so flags may be
   * interspersed anywhere without failing the match. Positional values are
   * compared case-sensitively (unlike flag values) because they are file
   * paths and issue identifiers the prompt states verbatim.
   */
  positionals?: RequiredPositional[]
  /** Flags of this subcommand that consume a value (stripped with it) */
  valueFlags?: string[]
  /** Flags of this subcommand that take no value */
  booleanFlags?: string[]
}

export interface ApiExpectation {
  route: "api"
  /** The query text (argv + stdin) must contain one of these markers */
  queryMarkers: string[]
}

export interface EvalCase {
  id: string
  family: string
  variant: "development" | "holdout" | "control"
  prompt: string
  expect: CliExpectation | ApiExpectation
}

const PREAMBLE =
  "You are working in a Linear workspace for the company Acme. " +
  "Teams: ENG (Engineering) and OPS (Operations). " +
  "Use the installed linear-cli skill. " +
  "Actually run the commands needed to do the task, then report the result briefly."

export const CASES: EvalCase[] = [
  {
    id: "query-development",
    family: "query",
    variant: "development",
    prompt:
      `${PREAMBLE} Task: list the started issues in the project "Mobile App Revamp" on the ENG team.`,
    expect: {
      route: "cli",
      routePrefixes: [
        ["issue", "query"],
        ["issue", "list"],
        ["issue", "mine"],
        [
          "issue",
          "l",
        ],
        ["issue", "q"],
      ],
      argsPrefix: ["issue", "query"],
      requiredArgs: [
        { flags: ["--project"], value: "Mobile App Revamp" },
        { flags: ["--state", "-s"], value: "started" },
      ],
    },
  },
  {
    id: "query-holdout",
    family: "query",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: show the unassigned issues on the OPS team that are in backlog or triage and were updated after 2026-06-01, as JSON.`,
    expect: {
      route: "cli",
      routePrefixes: [
        ["issue", "query"],
        ["issue", "list"],
        ["issue", "mine"],
        [
          "issue",
          "l",
        ],
        ["issue", "q"],
      ],
      argsPrefix: ["issue", "query"],
      requiredArgs: [
        { flags: ["--unassigned", "-U"] },
        { flags: ["--updated-after"], value: "2026-06-01" },
        { flags: ["--state", "-s"], value: "backlog" },
        { flags: ["--state", "-s"], value: "triage" },
        { flags: ["--json", "-j"] },
      ],
    },
  },
  {
    id: "create-development",
    family: "create",
    variant: "development",
    prompt:
      `${PREAMBLE} Task: create a new issue on the ENG team titled "Migrate CI to Blacksmith runners", using the markdown in ./issue-description.md as the description.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "create"]],
      argsPrefix: ["issue", "create"],
      requiredArgs: [
        { flags: ["--title", "-t"], value: "Migrate CI to Blacksmith runners" },
        {
          flags: ["--description-file"],
          value: "issue-description.md",
          valueIsPathSuffix: true,
        },
      ],
    },
  },
  {
    id: "create-holdout",
    family: "create",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: create an issue titled "Document rate limit behavior" on the OPS team in the project "Data Pipeline", with the labels "docs" and "api", using ./alternate-description.md as the description.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "create"]],
      argsPrefix: ["issue", "create"],
      requiredArgs: [
        { flags: ["--title", "-t"], value: "Document rate limit behavior" },
        {
          flags: ["--description-file"],
          value: "alternate-description.md",
          valueIsPathSuffix: true,
        },
        { flags: ["--label", "-l"], value: "docs" },
        { flags: ["--label", "-l"], value: "api" },
        { flags: ["--project"], value: "Data Pipeline" },
      ],
    },
  },
  {
    id: "update-development",
    family: "update",
    variant: "development",
    prompt:
      `${PREAMBLE} Task: move issue ENG-101 to the "In Review" state and assign it to priya.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "update"]],
      argsPrefix: ["issue", "update"],
      requiredArgs: [
        { flags: ["--state", "-s"], value: "In Review" },
        { flags: ["--assignee", "-a"], value: "priya" },
      ],
    },
  },
  {
    id: "update-holdout",
    family: "update",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: unassign issue OPS-31, move it into the project "Data Pipeline", and set its labels to "infra" and "security".`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "update"]],
      argsPrefix: ["issue", "update"],
      requiredArgs: [
        { flags: ["--unassign"] },
        { flags: ["--project"], value: "Data Pipeline" },
        { flags: ["--label", "-l"], value: "infra" },
        { flags: ["--label", "-l"], value: "security" },
      ],
    },
  },
  {
    id: "comment-development",
    family: "comment",
    variant: "development",
    prompt:
      `${PREAMBLE} Task: add the contents of ./comment.md as a comment on issue ENG-107.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "comment", "add"]],
      argsPrefix: ["issue", "comment", "add"],
      requiredArgs: [
        {
          flags: ["--body-file"],
          value: "comment.md",
          valueIsPathSuffix: true,
        },
      ],
    },
  },
  {
    id: "comment-holdout",
    family: "comment",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: post the markdown in ./alternate-comment.md as a comment on issue OPS-44.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "comment", "add"]],
      argsPrefix: ["issue", "comment", "add"],
      requiredArgs: [
        {
          flags: ["--body-file"],
          value: "alternate-comment.md",
          valueIsPathSuffix: true,
        },
      ],
    },
  },
  {
    id: "inspect-development",
    family: "inspect",
    variant: "development",
    prompt: `${PREAMBLE} Task: show the details of issue ENG-112 as JSON.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "view"]],
      argsPrefix: ["issue", "view"],
      requiredArgs: [
        { flags: ["--json", "-j"] },
      ],
    },
  },
  {
    id: "inspect-holdout",
    family: "inspect",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: print just the URL of issue ENG-107 (the URL and nothing else).`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "url"], ["issue", "view"]],
      argsPrefix: ["issue", "url"],
      requiredArgs: [],
    },
  },
  {
    id: "image-development",
    family: "image",
    variant: "development",
    prompt:
      `${PREAMBLE} Task: attach the screenshot at ./screenshot.png to issue ENG-107 so it is visible on the issue.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "comment", "add"]],
      argsPrefix: ["issue", "comment", "add"],
      requiredArgs: [
        {
          flags: ["--attach", "-a"],
          value: "screenshot.png",
          valueIsPathSuffix: true,
        },
      ],
      positionals: [{ value: "ENG-107" }],
      valueFlags: [
        "--body",
        "-b",
        "--body-file",
        "--parent",
        "-p",
        "--attach",
        "-a",
      ],
      booleanFlags: ["--public"],
    },
  },
  {
    id: "image-holdout",
    family: "image",
    variant: "holdout",
    prompt:
      `${PREAMBLE} Task: add a comment to issue OPS-44 with the screenshot at ./screenshot.png visible inline.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "comment", "add"]],
      argsPrefix: ["issue", "comment", "add"],
      requiredArgs: [
        {
          flags: ["--attach", "-a"],
          value: "screenshot.png",
          valueIsPathSuffix: true,
        },
      ],
      positionals: [{ value: "OPS-44" }],
      valueFlags: [
        "--body",
        "-b",
        "--body-file",
        "--parent",
        "-p",
        "--attach",
        "-a",
      ],
      booleanFlags: ["--public"],
    },
  },
  {
    id: "control-sidebar-attachment",
    family: "control",
    variant: "control",
    prompt:
      `${PREAMBLE} Task: attach ./server.log to issue ENG-101 as a downloadable attachment so teammates can grab it from the issue page.`,
    expect: {
      route: "cli",
      routePrefixes: [["issue", "attach"]],
      argsPrefix: ["issue", "attach"],
      requiredArgs: [],
      positionals: [
        { value: "ENG-101" },
        { value: "server.log", valueIsPathSuffix: true },
      ],
      valueFlags: ["--title", "-t", "--comment", "-c"],
      booleanFlags: ["--public"],
    },
  },
  {
    id: "control-history",
    family: "control",
    variant: "control",
    prompt:
      `${PREAMBLE} Task: show the change history of issue ENG-101 — who changed what and when (state changes, assignee changes), with timestamps.`,
    expect: {
      route: "api",
      queryMarkers: ["history"],
    },
  },
  {
    id: "control-subscribers",
    family: "control",
    variant: "control",
    prompt:
      `${PREAMBLE} Task: list the subscribers of issue ENG-101 with their names and email addresses.`,
    expect: {
      route: "api",
      queryMarkers: ["subscribers"],
    },
  },
]
