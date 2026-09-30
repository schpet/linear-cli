import {
  assertCaseCoverage,
  assertSameIds,
  caseDirectoryInventory,
} from "./test-support/corpus-coverage.ts"
import { nativeParserContract } from "./native-parser-contract.ts"
import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { CASE_ROOT_PARENT, prepareConfinement } from "./bwrap.ts"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases, resolveCase } from "./cases.ts"
import {
  assertProposalOutsideGoldens,
  loadCandidate,
  parseOptions,
} from "./main.ts"
import type { Program } from "./program.ts"
import {
  countReviewedDeviationPasses,
  countReviewedGraphqlUserAgentPasses,
  toReportCase,
} from "./report.ts"
import { runCorpus } from "./run.ts"
import {
  type CaseSpec,
  GraphQLFixtureSchema,
  type GraphQLFixtureSpec,
  parseCase,
  parseReviewedGolden,
  parseReviewedGoldenV2,
  SchemaError,
  ZeroRequestCandidateGraphQLSchema,
} from "./schema.ts"
import { testStatusHelper, validCase } from "./test-fixtures.ts"
import * as v from "valibot"

const CONTRACT = "rust-3.0.0-alpha.1"
const USER_AGENT = "schpet-linear-cli/3.0.0-alpha.1"
const GOLDEN_ID = "R01H-SYNTHETIC"

const C037_ID = "c037-first-page-more-json"
const C037_DEVIATION = "C037P02-SYNTHETIC"

function getPath(root: unknown, keys: readonly (string | number)[]): unknown {
  let value = root
  for (const key of keys) {
    if (typeof value !== "object" || value == null) {
      throw new Error(`test path ${keys.join(".")} is absent`)
    }
    value = Reflect.get(value, key)
  }
  return value
}

function setPath(
  root: unknown,
  keys: readonly (string | number)[],
  value: unknown,
): void {
  const parent = getPath(root, keys.slice(0, -1))
  if (typeof parent !== "object" || parent == null) {
    throw new Error(`test path ${keys.join(".")} is absent`)
  }
  Reflect.set(parent, keys[keys.length - 1], value)
}

function changeV2Document(
  golden: Record<string, unknown>,
  change: (document: string) => string,
): void {
  const path = [
    "candidate",
    "graphqlPages",
    "appendedSteps",
    0,
    "operation",
    "document",
  ]
  const document = getPath(golden, path)
  if (typeof document !== "string") throw new Error("test query is missing")
  setPath(golden, path, change(document))
}

function getV2Steps(golden: Record<string, unknown>): unknown[] {
  const steps = getPath(golden, ["candidate", "graphqlPages", "appendedSteps"])
  if (!Array.isArray(steps)) throw new Error("test pages are missing")
  return steps
}

async function withC037V2Corpus(
  fn: (
    dir: string,
    write: (
      golden: Record<string, unknown>,
      mutate?: (spec: Record<string, unknown>) => void,
    ) => Promise<void>,
    validGolden: () => Record<string, unknown>,
  ) => Promise<void>,
  caseId = C037_ID,
): Promise<void> {
  const dir = await Deno.makeTempDir({ prefix: "linear-reviewed-v2-" })
  const root = join(dir, "rust-goldens", CONTRACT)
  await Deno.mkdir(root, { recursive: true })
  const original = JSON.parse(
    await Deno.readTextFile(
      new URL(`./c037-frozen-cases/${caseId}.json`, import.meta.url),
    ),
  )
  const frozenStep = original.graphql.groups[0].steps[0]
  const document = frozenStep.operation.document.replace(
    "$includeArchived: Boolean)",
    "$includeArchived: Boolean, $after: String)",
  ).replace(
    "includeArchived: $includeArchived)",
    "includeArchived: $includeArchived, after: $after)",
  )
  const validGolden = () => {
    const second = structuredClone(frozenStep.response.data)
    if (second.initiatives.nodes.length > 0) {
      second.initiatives.nodes[0].id = "00000000-0000-4000-9000-000000000050"
    }
    const repeat = caseId === "c037-first-page-repeat-cursor-proposal"
    const cursor = frozenStep.response.data.initiatives.pageInfo.endCursor
    second.initiatives.pageInfo = repeat
      ? { hasNextPage: true, endCursor: "cycle-b" }
      : { hasNextPage: false, endCursor: null }
    const appendedSteps = [{
      id: "initiative-page-2",
      operation: {
        document,
        variables: {
          ...frozenStep.operation.variables,
          after: cursor,
        },
      },
      response: { kind: "data", data: second },
    }]
    if (repeat) {
      const third = structuredClone(second)
      third.initiatives.pageInfo = { hasNextPage: true, endCursor: cursor }
      appendedSteps.push({
        id: "initiative-page-3",
        operation: {
          document,
          variables: { ...frozenStep.operation.variables, after: "cycle-b" },
        },
        response: { kind: "data", data: third },
      })
    }
    const result = {
      formatVersion: 2,
      caseId,
      deviationId: C037_DEVIATION,
      contract: CONTRACT,
      approvedSurfaces: ["graphql-fixture", "graphql-user-agent"],
      candidate: {
        graphqlUserAgent: USER_AGENT,
        graphqlPages: {
          kind: "append-initiative-pages",
          retainedSteps: ["initiatives"],
          appendedSteps,
          expectedRequests: 1 + appendedSteps.length,
        },
      },
    }
    if (repeat) {
      result.approvedSurfaces = [
        "exit",
        "stdout",
        "stderr",
        "graphql-fixture",
        "graphql-user-agent",
      ]
      setPath(result, ["candidate", "expected"], {
        exit: { code: 1 },
        stdout: { utf8: "" },
        stderr: { utf8: "✗ repeated cursor\n" },
        fileEffects: [],
      })
    }
    return result
  }
  const write = async (
    golden: Record<string, unknown>,
    mutate?: (spec: Record<string, unknown>) => void,
  ) => {
    const spec = structuredClone(original)
    mutate?.(spec)
    const raw = JSON.stringify(golden, null, 2) + "\n"
    await Deno.writeTextFile(join(root, `${caseId}.json`), raw)
    spec.deviation = {
      id: C037_DEVIATION,
      contract: CONTRACT,
      sha256: await sha256Hex(new TextEncoder().encode(raw)),
    }
    await Deno.writeTextFile(join(dir, `${caseId}.json`), JSON.stringify(spec))
  }
  try {
    await fn(dir, write, validGolden)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("v2 pagination preserves frozen first page and derives validated candidate pages", async () => {
  await withC037V2Corpus(async (dir, write, validGolden) => {
    const golden = validGolden()
    await write(golden)
    const [loaded] = await loadCases(
      dir,
      new Set(["linear initiative list"]),
      undefined,
      CONTRACT,
    )
    const frozen = structuredClone(loaded.spec)
    const view = candidateCaseView(loaded)
    assertEquals(loaded.spec, frozen)
    assertEquals(loaded.spec.graphql?.expectedRequests, 1)
    assertEquals(view.spec.graphql?.expectedRequests, 2)
    assertEquals(view.runtimeUserAgent, USER_AGENT)
    assertEquals(view.golden, null)
    assertEquals(view.goldenV2?.spec.formatVersion, 2)
    const group = view.spec.graphql?.groups[0]
    assert(group?.mode === "ordered")
    assertEquals(
      group.steps[0],
      loaded.spec.graphql?.groups[0].mode === "ordered"
        ? loaded.spec.graphql.groups[0].steps[0]
        : null,
    )
    assertEquals(group.steps[1].kind, "graphql")
    if (group.steps[1].kind !== "graphql") throw new Error("expected query")
    assertEquals(group.steps[1].operation.variables?.after, "page-a")
    assertEquals(
      group.steps[1].response,
      getPath(golden, [
        "candidate",
        "graphqlPages",
        "appendedSteps",
        0,
        "response",
      ]),
    )
    assertEquals(
      group.steps[1].identity,
      group.steps[0].kind === "graphql" ? group.steps[0].identity : null,
    )
    assertThrows(
      () => candidateCaseView({ ...loaded }),
      SchemaError,
      "unvalidated v2",
    )
    assertThrows(
      () =>
        candidateCaseView({
          ...loaded,
          golden: {
            spec: parseReviewedGolden({
              ...golden,
              formatVersion: 1,
              candidate: {},
            }),
            sha256: "0".repeat(64),
          },
        }),
      SchemaError,
      "both golden versions",
    )
    assertThrows(() => parseReviewedGolden(golden), SchemaError)
    assertThrows(
      () => parseReviewedGoldenV2({ ...golden, formatVersion: 1 }),
      SchemaError,
    )
    assertEquals(parseReviewedGoldenV2(golden).formatVersion, 2)
  })
})

Deno.test("v2 pagination loads all four eligible frozen initiative cases", async () => {
  for (
    const caseId of [
      "c037-first-page-more-json",
      "c037-first-page-more-text",
      "c037-empty-first-page-has-next",
      "c037-first-page-repeat-cursor-proposal",
    ]
  ) {
    await withC037V2Corpus(async (dir, write, validGolden) => {
      const value = validGolden()
      await write(value)
      const [loaded] = await loadCases(
        dir,
        new Set(["linear initiative list"]),
        undefined,
        CONTRACT,
      )
      assertEquals(loaded.spec.id, caseId)
      assertEquals(loaded.spec.graphql?.expectedRequests, 1)
      assertEquals(
        candidateCaseView(loaded).spec.graphql?.expectedRequests,
        caseId === "c037-first-page-repeat-cursor-proposal" ? 3 : 2,
      )
    }, caseId)
  }
})

Deno.test("v2 pagination rejects malformed pages and changed frozen inputs at load time", async () => {
  await withC037V2Corpus(async (dir, write, validGolden) => {
    const load = () =>
      loadCases(
        dir,
        new Set(["linear initiative list", "linear initiative view"]),
        undefined,
        CONTRACT,
      )
    const cases: Array<[(golden: Record<string, unknown>) => void, string]> = [
      [(g) => {
        g.formatVersion = 3
      }, "formatVersion"],
      [(g) => {
        g.caseId = "other"
      }, "caseId"],
      [(g) => {
        setPath(g, ["candidate", "argv"], ["initiative", "list"])
      }, "argv"],
      [(g) => {
        setPath(g, ["candidate", "graphql"], { steps: [] })
      }, "graphql"],
      [(g) => {
        setPath(g, ["candidate", "graphqlPages", "retainedSteps"], ["other"])
      }, "retained step"],
      [(g) => {
        setPath(g, ["candidate", "graphqlPages", "retainedSteps"], [
          "initiatives",
          "junk",
        ])
      }, "retainedSteps"],
      [(g) => {
        setPath(g, ["candidate", "graphqlPages", "retainedSteps"], [
          "initiatives",
          "initiatives",
        ])
      }, "retainedSteps"],
      [(g) => {
        setPath(g, ["candidate", "graphqlPages", "expectedRequests"], 3)
      }, "count"],
      [(g) => {
        setPath(g, ["candidate", "graphqlPages", "appendedSteps"], [
          ...getV2Steps(g),
          ...getV2Steps(g),
        ])
      }, "appended page count"],
      [(g) => {
        setPath(
          g,
          ["candidate", "graphqlPages", "appendedSteps", 0, "id"],
          "initiatives",
        )
      }, "duplicate"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "operation",
          "variables",
          "after",
        ], "wrong")
      }, "cursor"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "operation",
          "variables",
          "first",
        ], 1)
      }, "variables"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "operation",
          "exactOrigins",
        ], [])
      }, "exactOrigins"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "operation",
          "allowExtraTypename",
        ], true)
      }, "allowExtraTypename"],
      [(g) => {
        changeV2Document(
          g,
          (document) => document.replace(", after: $after)", ")"),
        )
      }, "needs exactly after"],
      [(g) => {
        changeV2Document(
          g,
          (document) => document.replace("$after: String)", "$after: String!)"),
        )
      }, "needs exactly after"],
      [(g) => {
        changeV2Document(
          g,
          (document) =>
            document.replace("after: $after)", "after: $after, first: 50)"),
        )
      }, "changes the frozen selection"],
      [(g) => {
        changeV2Document(g, (document) => document.replace("archivedAt", ""))
      }, "changes the frozen selection"],
      [(g) => {
        setPath(
          g,
          [
            "candidate",
            "graphqlPages",
            "appendedSteps",
            0,
            "operation",
            "document",
          ],
          `${
            getPath(g, [
              "candidate",
              "graphqlPages",
              "appendedSteps",
              0,
              "operation",
              "document",
            ])
          } query Another { __typename }`,
        )
      }, "one operation"],
      [(g) => {
        setPath(
          g,
          [
            "candidate",
            "graphqlPages",
            "appendedSteps",
            0,
            "operation",
            "document",
          ],
          `${
            getPath(g, [
              "candidate",
              "graphqlPages",
              "appendedSteps",
              0,
              "operation",
              "document",
            ])
          } fragment X on Query { __typename }`,
        )
      }, "fragments"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "operation",
          "document",
        ], "query {")
      }, "invalid appended query"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "response",
          "data",
          "initiatives",
          "nodes",
          0,
          "name",
        ], undefined)
      }, "projected response"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "response",
          "data",
          "initiatives",
          "nodes",
          0,
          "health",
        ], "invalid-health")
      }, "projected response"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "response",
          "data",
          "initiatives",
          "nodes",
          0,
        ], { "$record": "unknown" })
      }, "record reference"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "response",
          "data",
          "initiatives",
          "pageInfo",
          "hasNextPage",
        ], true)
      }, "terminal"],
      [(g) => {
        setPath(g, [
          "candidate",
          "graphqlPages",
          "appendedSteps",
          0,
          "response",
          "data",
          "extra",
        ], {})
      }, "only initiatives"],
      [(g) => {
        setPath(g, ["approvedSurfaces"], ["graphql-fixture"])
      }, "approvedSurfaces"],
      [(g) => {
        setPath(g, ["candidate", "graphqlUserAgent"], undefined)
      }, "graphqlUserAgent"],
    ]
    for (const [change, message] of cases) {
      const value = validGolden()
      change(value)
      await write(value)
      await assertRejects(load, SchemaError, message)
    }
    await write(validGolden(), (spec) => {
      setPath(spec, [
        "graphql",
        "groups",
        0,
        "steps",
        0,
        "response",
        "data",
        "initiatives",
        "pageInfo",
        "endCursor",
      ], null)
    })
    await assertRejects(load, SchemaError, "continuation cursor")
    await write(validGolden(), (spec) => {
      setPath(spec, [
        "graphql",
        "groups",
        0,
        "steps",
        0,
        "response",
        "data",
        "initiatives",
        "pageInfo",
        "hasNextPage",
      ], false)
    })
    await assertRejects(load, SchemaError, "continuation cursor")
    await write(validGolden(), (spec) => {
      setPath(
        spec,
        ["graphql", "groups", 0, "steps", 0, "operation", "document"],
        "query GetInitiatives { initiatives { pageInfo { hasNextPage endCursor } } }",
      )
    })
    await assertRejects(load, SchemaError, "invalid fixture expectation")
    await write(validGolden(), (spec) => {
      setPath(spec, [
        "graphql",
        "groups",
        0,
        "steps",
        0,
        "response",
        "data",
        "extra",
      ], {})
    })
    await assertRejects(load, SchemaError, "only initiatives")
    await write(validGolden(), (spec) => {
      spec.route = "linear initiative view"
    })
    await assertRejects(load, SchemaError, "v2 needs one")
  })
})

Deno.test("v2 repeated cursor requires exactly A to B to A", async () => {
  await withC037V2Corpus(async (dir, write, validGolden) => {
    const load = () =>
      loadCases(dir, new Set(["linear initiative list"]), undefined, CONTRACT)
    const valid = validGolden()
    await write(valid)
    const [loaded] = await load()
    assertEquals(candidateCaseView(loaded).spec.graphql?.expectedRequests, 3)
    assertEquals(
      candidateCaseView(loaded).spec.expected,
      loaded.goldenV2?.spec.candidate.expected,
    )
    const wrongThird = validGolden()
    setPath(wrongThird, [
      "candidate",
      "graphqlPages",
      "appendedSteps",
      1,
      "response",
      "data",
      "initiatives",
      "pageInfo",
      "endCursor",
    ], "cycle-c")
    await write(wrongThird)
    await assertRejects(load, SchemaError, "A→B→A")
    const wrongSecond = validGolden()
    setPath(wrongSecond, [
      "candidate",
      "graphqlPages",
      "appendedSteps",
      0,
      "response",
      "data",
      "initiatives",
      "pageInfo",
      "endCursor",
    ], "cycle-a")
    await write(wrongSecond)
    await assertRejects(load, SchemaError, "A→B→A")
    const wrongAfter = validGolden()
    setPath(wrongAfter, [
      "candidate",
      "graphqlPages",
      "appendedSteps",
      1,
      "operation",
      "variables",
      "after",
    ], "cycle-a")
    await write(wrongAfter)
    await assertRejects(load, SchemaError, "preceding cursor")
    const noFailureExpectation = validGolden()
    setPath(noFailureExpectation, ["candidate", "expected"], undefined)
    setPath(noFailureExpectation, ["approvedSurfaces"], [
      "graphql-fixture",
      "graphql-user-agent",
    ])
    await write(noFailureExpectation)
    await assertRejects(load, SchemaError, "exit nonzero with empty stdout")
  }, "c037-first-page-repeat-cursor-proposal")
})

Deno.test("v2 candidate consumes two ordered pages and reports reviewed provenance", async () => {
  await withC037V2Corpus(async (dir, write, validGolden) => {
    const value = validGolden()
    const source = parseCase(JSON.parse(
      await Deno.readTextFile(
        new URL(`./c037-frozen-cases/${C037_ID}.json`, import.meta.url),
      ),
    ))
    const firstNodes = getPath(source, [
      "graphql",
      "groups",
      0,
      "steps",
      0,
      "response",
      "data",
      "initiatives",
      "nodes",
    ])
    const secondNodes = getPath(value, [
      "candidate",
      "graphqlPages",
      "appendedSteps",
      0,
      "response",
      "data",
      "initiatives",
      "nodes",
    ])
    const finalPageInfo = getPath(value, [
      "candidate",
      "graphqlPages",
      "appendedSteps",
      0,
      "response",
      "data",
      "initiatives",
      "pageInfo",
    ])
    if (!Array.isArray(firstNodes) || !Array.isArray(secondNodes)) {
      throw new Error("synthetic pages need node arrays")
    }
    const mergedText = JSON.stringify(
      {
        nodes: [...firstNodes, ...secondNodes],
        pageInfo: finalPageInfo,
      },
      null,
      2,
    ) + "\n"
    setPath(value, ["candidate", "expected"], {
      ...source.expected,
      stdout: { utf8: mergedText },
    })
    setPath(value, ["approvedSurfaces"], [
      "stdout",
      "graphql-fixture",
      "graphql-user-agent",
    ])
    await write(value)
    const [loaded] = await loadCases(
      dir,
      new Set(["linear initiative list"]),
      undefined,
      CONTRACT,
    )
    const frozenGroup = loaded.spec.graphql?.groups[0]
    const candidateGroup = candidateCaseView(loaded).spec.graphql?.groups[0]
    assertEquals(loaded.spec.expected, source.expected)
    assertEquals(loaded.goldenV2?.spec.candidate.expected?.stdout, {
      utf8: mergedText,
    })
    assert(mergedText !== getPath(source, ["expected", "stdout", "utf8"]))
    assert(
      frozenGroup?.mode === "ordered" && candidateGroup?.mode === "ordered",
    )
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-v2-pages-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement: await prepareConfinement({
          denoDir,
          statusHelper: await testStatusHelper(runDir),
        }),
        sandboxParent: runDir,
      }
      const body = (step: typeof frozenGroup.steps[number]) => {
        if (step.kind !== "graphql") throw new Error("expected GraphQL")
        return JSON.stringify({
          query: step.operation.document,
          variables: step.operation.variables,
        })
      }
      const script = async (
        name: string,
        agent: string,
        requests: string[],
        merge: boolean,
      ): Promise<Program> => {
        const path = join(runDir, name)
        const lines = requests.map((request, index) =>
          `p${
            index + 1
          }=$(/usr/bin/curl --silent --show-error --noproxy '*' --request POST --header 'content-type: application/json' --header 'authorization: lin_api_fake' --header 'user-agent: ${agent}' --data-raw '${request}' "$LINEAR_GRAPHQL_ENDPOINT")`
        )
        lines.push(
          merge
            ? '/usr/bin/jq -n --argjson first "$p1" --argjson second "${p2:-null}" \'{nodes: ($first.data.initiatives.nodes + $second.data.initiatives.nodes), pageInfo: $second.data.initiatives.pageInfo}\''
            : "/usr/bin/jq -n --argjson first \"$p1\" '$first.data.initiatives'",
        )
        await Deno.writeTextFile(
          path,
          `#!/bin/sh\nset -eu\n${lines.join("\n")}\n`,
          { mode: 0o755 },
        )
        return { kind: "executable", path }
      }
      const first = body(frozenGroup.steps[0])
      const second = body(candidateGroup.steps[1])
      const baseline = await script("baseline.sh", "schpet-linear-cli/2.6.0", [
        first,
      ], false)
      const two = await script("two.sh", USER_AGENT, [first, second], true)
      const one = await script("one.sh", USER_AGENT, [first], true)
      const extra = await script("extra.sh", USER_AGENT, [
        first,
        second,
        second,
      ], true)
      const wrongCursor = await script("wrong-cursor.sh", USER_AGENT, [
        first,
        JSON.stringify({
          query: candidateGroup.steps[1].kind === "graphql"
            ? candidateGroup.steps[1].operation.document
            : "",
          variables: {
            ...(candidateGroup.steps[1].kind === "graphql"
              ? candidateGroup.steps[1].operation.variables
              : {}),
            after: "wrong-cursor",
          },
        }),
      ], true)
      const wrongDocument = await script("wrong-document.sh", USER_AGENT, [
        first,
        JSON.stringify({
          query: candidateGroup.steps[1].kind === "graphql"
            ? candidateGroup.steps[1].operation.document.replace(
              "archivedAt",
              "",
            )
            : "",
          variables: candidateGroup.steps[1].kind === "graphql"
            ? candidateGroup.steps[1].operation.variables
            : {},
        }),
      ], true)
      const selected = new Set(["linear initiative list"])
      const run = async (program: Program) => {
        const [result] = await runCorpus([loaded], baseline, {
          name: "synthetic Rust candidate",
          contract: CONTRACT,
          program,
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.baseline.mismatches, [])
        assertEquals(result.baseline.fixture?.graphqlRequests, 1)
        assertEquals(result.reviewedDeviation?.id, C037_DEVIATION)
        assertEquals(result.reviewedDeviation?.sha256, loaded.goldenV2?.sha256)
        return result
      }
      const pass = await run(two)
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.candidate?.fixture?.graphqlRequests, 2)
      assertEquals(
        pass.candidate?.observation.stdoutBytes,
        new TextEncoder().encode(mergedText).length,
      )
      const missing = await run(one)
      assertEquals(missing.status, "fail")
      assert(
        (missing.candidate?.mismatches ?? []).some((mismatch) =>
          mismatch.surface === "fixture"
        ),
      )
      const unexpected = await run(extra)
      assertEquals(unexpected.status, "fail")
      assert(
        (unexpected.candidate?.mismatches ?? []).some((mismatch) =>
          mismatch.surface === "fixture"
        ),
      )
      for (const program of [wrongCursor, wrongDocument]) {
        const wrong = await run(program)
        assertEquals(wrong.status, "fail")
        assert(
          (wrong.candidate?.mismatches ?? []).some((mismatch) =>
            mismatch.surface === "fixture"
          ),
        )
      }
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})

function golden(candidate: Record<string, unknown>, surfaces: string[]) {
  return {
    formatVersion: 1,
    caseId: "sample",
    deviationId: GOLDEN_ID,
    contract: CONTRACT,
    approvedSurfaces: surfaces,
    candidate,
  }
}

async function withCorpus(
  fn: (
    dir: string,
    write: (
      value: Record<string, unknown>,
      caseSpec?: Record<string, unknown>,
    ) => Promise<void>,
  ) => Promise<void>,
): Promise<void> {
  const dir = await Deno.makeTempDir({ prefix: "linear-reviewed-golden-" })
  const root = join(dir, "rust-goldens", CONTRACT)
  await Deno.mkdir(root, { recursive: true })
  const write = async (
    value: Record<string, unknown>,
    caseSpec = validCase(),
  ) => {
    const raw = JSON.stringify(value, null, 2) + "\n"
    await Deno.writeTextFile(join(root, "sample.json"), raw)
    const spec = { ...caseSpec }
    spec.deviation = {
      id: GOLDEN_ID,
      contract: CONTRACT,
      sha256: await sha256Hex(new TextEncoder().encode(raw)),
    }
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
  }
  try {
    await fn(dir, write)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

async function frozenGraphQLCase(id: string): Promise<Record<string, unknown>> {
  const spec = JSON.parse(
    await Deno.readTextFile(
      new URL(`./c016-frozen-cases/${id}.json`, import.meta.url),
    ),
  )
  spec.id = "sample"
  spec.route = "linear"
  return spec
}

function graphqlGolden(
  steps: Array<{ id: string; variables?: Record<string, unknown> }>,
  surfaces = ["graphql-fixture", "graphql-user-agent"],
): Record<string, unknown> {
  return golden({
    graphql: { steps },
    graphqlUserAgent: USER_AGENT,
  }, surfaces)
}

Deno.test("reviewed GraphQL delta keeps a frozen baseline and derives only a request prefix", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-cursor-null")
    await write(graphqlGolden([{ id: "labels-first" }]), spec)
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    const frozen = structuredClone(loaded.spec.graphql)
    const candidate = candidateCaseView(loaded)
    assertEquals(loaded.spec.graphql, frozen)
    assertEquals(loaded.spec.graphql?.expectedRequests, 2)
    assertEquals(candidate.spec.graphql?.expectedRequests, 1)
    assertEquals(candidate.spec.graphql?.groups[0].mode, "ordered")
    if (candidate.spec.graphql?.groups[0].mode !== "ordered") {
      throw new Error("candidate GraphQL group must be ordered")
    }
    assertEquals(
      candidate.spec.graphql.groups[0].steps.map((step) => step.id),
      [
        "labels-first",
      ],
    )
    assertEquals(candidate.runtimeUserAgent, USER_AGENT)
  })
})

Deno.test("reviewed GraphQL delta can replace only variables on a retained request", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-default-team")
    await write(
      graphqlGolden([{ id: "labels-first", variables: { first: 100 } }]),
      spec,
    )
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    const original = loaded.spec.graphql
    const candidate = candidateCaseView(loaded).spec.graphql
    assert(original != null && candidate != null)
    assertEquals(original.expectedRequests, 1)
    assertEquals(candidate.expectedRequests, 1)
    assertEquals(candidate.path, original.path)
    assertEquals(candidate.initialRecords, original.initialRecords)
    assertEquals(candidate.expectedRecords, original.expectedRecords)
    if (
      original.groups[0].mode !== "ordered" ||
      candidate.groups[0].mode !== "ordered"
    ) {
      throw new Error("fixture must be ordered")
    }
    const before = original.groups[0].steps[0]
    const after = candidate.groups[0].steps[0]
    assert(before.kind === "graphql" && after.kind === "graphql")
    assertEquals(after.operation, {
      ...before.operation,
      variables: { first: 100 },
    })
    assertEquals(after.identity, before.identity)
    assertEquals(after.response, before.response)
    assertEquals(after.effects, before.effects)
  })
})

Deno.test("GraphQL deltas reject non-prefix, redundant, unsafe and unapproved changes at load time", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-cursor-null")
    const load = () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT)
    const rejects: Array<[
      Record<string, unknown>,
      string,
    ]> = [
      [
        graphqlGolden([]),
        "zero-request GraphQL delta needs exactly one frozen query step",
      ],
      [graphqlGolden([{ id: "labels-cursor-one" }]), "not the frozen prefix"],
      [graphqlGolden([{ id: "unknown" }]), "not the frozen prefix"],
      [
        graphqlGolden([{ id: "labels-first" }, { id: "labels-first" }]),
        "not the frozen prefix",
      ],
      [
        graphqlGolden([
          { id: "labels-first" },
          { id: "labels-cursor-one" },
          { id: "extra" },
        ]),
        "must be a prefix",
      ],
      [
        graphqlGolden([
          { id: "labels-first" },
          { id: "labels-cursor-one" },
        ]),
        "does not change",
      ],
      [
        graphqlGolden([{ id: "labels-first", variables: { first: 100 } }]),
        "variables are unchanged",
      ],
      [
        graphqlGolden([{ id: "labels-first", variables: { first: "wrong" } }]),
        "Int",
      ],
      [
        graphqlGolden([{
          id: "labels-first",
          variables: { first: 100, after: "{{home}}" },
        }]),
        "placeholders",
      ],
      [
        graphqlGolden([{
          id: "labels-first",
          variables: { first: 100, after: "lin_api_real" },
        }]),
        "credentials",
      ],
      [
        graphqlGolden([{ id: "labels-first" }], ["graphql-user-agent"]),
        "approvedSurfaces",
      ],
      [
        golden({ graphql: { steps: [{ id: "labels-first" }] } }, [
          "graphql-fixture",
        ]),
        "requires the Rust User-Agent",
      ],
    ]
    for (const [value, message] of rejects) {
      await write(value, spec)
      await assertRejects(load, SchemaError, message)
    }
    await write(graphqlGolden([{ id: "labels-first" }]), spec)
    const casePath = join(dir, "sample.json")
    const pinned = JSON.parse(await Deno.readTextFile(casePath))
    pinned.deviation.sha256 = "0".repeat(64)
    await Deno.writeTextFile(casePath, JSON.stringify(pinned))
    await assertRejects(load, SchemaError, "SHA-256 differs")
  })
})

Deno.test("GraphQL delta schema is closed and requires a GraphQL fixture", async () => {
  for (
    const candidate of [
      {
        graphql: { steps: [{ id: "labels-first", response: {} }] },
        graphqlUserAgent: USER_AGENT,
      },
      {
        graphql: { steps: [{ id: "labels-first" }], document: "query X { x }" },
        graphqlUserAgent: USER_AGENT,
      },
    ]
  ) {
    assertThrows(
      () =>
        parseReviewedGolden(
          golden(candidate, ["graphql-fixture", "graphql-user-agent"]),
        ),
      SchemaError,
    )
  }
  await withCorpus(async (dir, write) => {
    await write(graphqlGolden([{ id: "labels-first" }]))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "needs a GraphQL fixture",
    )
  })
})

Deno.test("GraphQL delta rejects missing original variables and changed source records", async () => {
  await withCorpus(async (dir, write) => {
    const source = await frozenGraphQLCase("c016-cursor-null")
    const withoutVariables = structuredClone(parseCase(source))
    const group = withoutVariables.graphql?.groups[0]
    if (group?.mode !== "ordered" || group.steps[0].kind !== "graphql") {
      throw new Error("missing frozen GraphQL step")
    }
    const first = group.steps[0]
    delete first.operation.variables
    await write(
      graphqlGolden([{ id: "labels-first", variables: { first: 100 } }]),
      withoutVariables,
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "cannot add a variables key",
    )
    const changedRecords = structuredClone(parseCase(source))
    if (changedRecords.graphql == null) {
      throw new Error("missing frozen fixture")
    }
    changedRecords.graphql.expectedRecords = { unexpected: true }
    await write(graphqlGolden([{ id: "labels-first" }]), changedRecords)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "effect-free ordered group",
    )
    const mutation = structuredClone(parseCase(source))
    const mutationGroup = mutation.graphql?.groups[0]
    if (
      mutationGroup?.mode !== "ordered" ||
      mutationGroup.steps[0].kind !== "graphql"
    ) throw new Error("missing frozen GraphQL step")
    const mutationStep = mutationGroup.steps[0]
    mutationStep.operation = { document: "mutation { __typename }" }
    mutationStep.response = {
      kind: "data",
      data: { __typename: "Mutation" },
    }
    await write(graphqlGolden([{ id: "labels-first" }]), mutation)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "effect-free query steps",
    )
    const droppedMutation = structuredClone(parseCase(source))
    const droppedGroup = droppedMutation.graphql?.groups[0]
    if (
      droppedGroup?.mode !== "ordered" ||
      droppedGroup.steps[1].kind !== "graphql"
    ) throw new Error("missing frozen GraphQL suffix step")
    droppedGroup.steps[1].operation = {
      document: "mutation { __typename }",
    }
    droppedGroup.steps[1].response = {
      kind: "data",
      data: { __typename: "Mutation" },
    }
    await write(graphqlGolden([{ id: "labels-first" }]), droppedMutation)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "effect-free query steps",
    )
    const splitGroups = structuredClone(parseCase(source))
    const split = splitGroups.graphql?.groups[0]
    if (splitGroups.graphql == null || split?.mode !== "ordered") {
      throw new Error("missing frozen ordered group")
    }
    splitGroups.graphql.groups = [
      { mode: "ordered", steps: [split.steps[0]] },
      { mode: "ordered", steps: [split.steps[1]] },
    ]
    await write(graphqlGolden([{ id: "labels-first" }]), splitGroups)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "effect-free ordered group",
    )
    const lanes = structuredClone(parseCase(source))
    const ordered = lanes.graphql?.groups[0]
    if (lanes.graphql == null || ordered?.mode !== "ordered") {
      throw new Error("missing frozen ordered group")
    }
    lanes.graphql.groups = [{
      mode: "lanes",
      timeoutMs: 1000,
      lanes: [
        { id: "first", steps: [ordered.steps[0]] },
        { id: "second", steps: [ordered.steps[1]] },
      ],
    }]
    await write(graphqlGolden([{ id: "labels-first" }]), lanes)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "effect-free ordered group",
    )
  })
})

Deno.test("candidate-only request prefix is enforced while the baseline keeps both pages", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-cursor-null")
    const parsed = parseCase(spec)
    const group = parsed.graphql?.groups[0]
    if (
      group?.mode !== "ordered" ||
      group.steps.some((step) => step.kind !== "graphql")
    ) {
      throw new Error("expected two frozen GraphQL steps")
    }
    const steps = group.steps
    const first = steps[0]
    const second = steps[1]
    if (first.kind !== "graphql" || second.kind !== "graphql") {
      throw new Error("expected GraphQL steps")
    }
    spec.expected = {
      exit: { code: 0 },
      stdout: { utf8: "ok" },
      stderr: { utf8: "" },
      fileEffects: [],
    }
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-graphql-delta-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement: await prepareConfinement({
          denoDir,
          statusHelper: await testStatusHelper(runDir),
        }),
        sandboxParent: runDir,
      }
      const script = async (
        name: string,
        userAgent: string,
        requests: string[],
      ): Promise<Program> => {
        const path = join(runDir, name)
        const lines = requests.map((body) =>
          `/usr/bin/curl --silent --show-error --noproxy '*' --request POST --header 'content-type: application/json' --header 'authorization: lin_api_fake' --header 'user-agent: ${userAgent}' --data-raw '${body}' "$LINEAR_GRAPHQL_ENDPOINT" >/dev/null`
        )
        await Deno.writeTextFile(
          path,
          `#!/bin/sh\n${lines.join("\n")}\nprintf ok\n`,
          { mode: 0o755 },
        )
        return { kind: "executable", path }
      }
      const body = (step: typeof first) => {
        if (step.kind !== "graphql") throw new Error("expected GraphQL")
        return JSON.stringify({
          query: step.operation.document,
          variables: step.operation.variables,
        })
      }
      const baseline = await script("baseline.sh", "schpet-linear-cli/2.6.0", [
        body(first),
        body(second),
      ])
      const candidate = await script("candidate.sh", USER_AGENT, [body(first)])
      const bothPages = await script("both-pages.sh", USER_AGENT, [
        body(first),
        body(second),
      ])
      const wrongVariables = await script("wrong-variables.sh", USER_AGENT, [
        JSON.stringify({
          query: first.operation.document,
          variables: { first: 100, after: "wrong" },
        }),
      ])
      const selected = new Set(["linear"])
      const run = async (
        value: Record<string, unknown>,
        program: Program,
      ) => {
        await write(value, spec)
        const [loaded] = await loadCases(dir, selected, undefined, CONTRACT)
        const [result] = await runCorpus([loaded], baseline, {
          name: "synthetic Rust candidate",
          contract: CONTRACT,
          program,
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.baseline.mismatches, [])
        assertEquals(result.baseline.fixture?.graphqlRequests, 2)
        return result
      }
      const shortened = graphqlGolden([{ id: "labels-first" }])
      const pass = await run(shortened, candidate)
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.candidate?.fixture?.graphqlRequests, 1)
      const replacement = { first: 100, after: "candidate-only" }
      const variableCandidate = await script(
        "candidate-variables.sh",
        USER_AGENT,
        [
          JSON.stringify({
            query: first.operation.document,
            variables: replacement,
          }),
        ],
      )
      const variablePass = await run(
        graphqlGolden([{ id: "labels-first", variables: replacement }]),
        variableCandidate,
      )
      assertEquals(
        variablePass.status,
        "pass",
        JSON.stringify(variablePass.candidate?.mismatches),
      )
      for (const program of [bothPages, wrongVariables]) {
        const failed = await run(shortened, program)
        assertEquals(failed.status, "fail")
        assert(
          failed.candidate?.mismatches.some((mismatch) =>
            mismatch.surface === "fixture"
          ),
        )
      }
      const missing = await run(
        golden({ graphqlUserAgent: USER_AGENT }, ["graphql-user-agent"]),
        candidate,
      )
      assertEquals(missing.status, "fail")
      assert(
        missing.candidate?.mismatches.some((mismatch) =>
          mismatch.surface === "fixture"
        ),
      )
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})

const DUMMY_VALUES = {
  home: "h",
  configHome: "c",
  cwd: "w",
  cwdRoot: "r",
  bin: "b",
  denoDir: "d",
  fixturePort: "0",
  referenceModuleUrl: "file:///reference",
}

Deno.test("zero-request GraphQL delta derives an empty candidate fixture from one frozen query", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-default-team")
    const load = async () => {
      const [loaded] = await loadCases(
        dir,
        new Set(["linear"]),
        undefined,
        CONTRACT,
      )
      return loaded
    }
    await write(graphqlGolden([]), spec)
    const loaded = await load()
    const frozen = structuredClone(loaded.spec)
    const candidate = candidateCaseView(loaded)
    assertEquals(loaded.spec, frozen)
    const original = loaded.spec.graphql
    assert(original != null && original.groups[0].mode === "ordered")
    assertEquals(original.expectedRequests, 1)
    assertEquals(original.groups[0].steps.length, 1)
    assertEquals(candidate.runtimeUserAgent, USER_AGENT)
    assertEquals(candidate.spec.graphql, {
      ...original,
      expectedRequests: 0,
      groups: [],
    })
    const resolved = resolveCase(candidate.spec, DUMMY_VALUES, USER_AGENT)
    assertEquals(resolved.graphql?.expectedRequests, 0)
    assertEquals(resolved.graphql?.groups, [])
    assertEquals(resolved.graphql?.path, original.path)
    // Baseline and no-User-Agent resolution stay on the strict frozen schema.
    assertThrows(() => resolveCase(candidate.spec, DUMMY_VALUES))
    assertEquals(
      resolveCase(loaded.spec, DUMMY_VALUES).graphql?.expectedRequests,
      1,
    )
    assertEquals(
      resolveCase(loaded.spec, DUMMY_VALUES, USER_AGENT).graphql
        ?.expectedRequests,
      1,
    )
    // Ordinary candidate surfaces combine with the zero delta and still need
    // exact approval.
    const expected = parseCase(spec).expected
    const stderr = {
      ...expected,
      stderr: { utf8: "✗ zero-request candidate\n" },
    }
    await write(
      golden({
        expected: stderr,
        graphql: { steps: [] },
        graphqlUserAgent: USER_AGENT,
      }, ["graphql-fixture", "graphql-user-agent", "stderr"]),
      spec,
    )
    assertEquals(
      candidateCaseView(await load()).spec.graphql?.expectedRequests,
      0,
    )
    await write(
      golden({
        expected: stderr,
        graphql: { steps: [] },
        graphqlUserAgent: USER_AGENT,
      }, ["graphql-fixture", "graphql-user-agent"]),
      spec,
    )
    await assertRejects(load, SchemaError, "approvedSurfaces")
    // Candidate case fields are still checked against the frozen fixture and
    // resolved before any child can run.
    await write(
      golden({
        expected: { ...expected, stderr: { utf8: "{{cwd}}" } },
        graphql: { steps: [] },
        graphqlUserAgent: USER_AGENT,
      }, ["graphql-fixture", "graphql-user-agent", "stderr"]),
      spec,
    )
    await assertRejects(load, SchemaError, "not declared in substitutions")
    await write(
      golden({
        expected: {
          ...expected,
          stdout: {
            mode: "close-after-bytes",
            count: 33,
            prefix: { utf8: "a".repeat(33) },
          },
        },
        graphql: { steps: [] },
        graphqlUserAgent: USER_AGENT,
      }, ["graphql-fixture", "graphql-user-agent", "stdout"]),
      { ...spec, outputCapBytes: 32 },
    )
    await assertRejects(
      load,
      SchemaError,
      "close-after-bytes count must not exceed outputCapBytes",
    )
  })
})

Deno.test("zero-request GraphQL delta rejects every shape except one frozen effect-free query", async () => {
  await withCorpus(async (dir, write) => {
    const load = () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT)
    const zero = graphqlGolden([])
    const one = parseCase(await frozenGraphQLCase("c016-default-team"))
    const two = parseCase(await frozenGraphQLCase("c016-cursor-null"))

    await write(zero, two)
    await assertRejects(
      load,
      SchemaError,
      "zero-request GraphQL delta needs exactly one frozen query step",
    )

    const mutation = structuredClone(one)
    const mutationGroup = mutation.graphql?.groups[0]
    if (
      mutationGroup?.mode !== "ordered" ||
      mutationGroup.steps[0].kind !== "graphql"
    ) throw new Error("missing frozen GraphQL step")
    mutationGroup.steps[0].operation = { document: "mutation { __typename }" }
    mutationGroup.steps[0].response = {
      kind: "data",
      data: { __typename: "Mutation" },
    }
    await write(zero, mutation)
    await assertRejects(load, SchemaError, "effect-free query steps")

    const droppedMutation = structuredClone(two)
    const droppedGroup = droppedMutation.graphql?.groups[0]
    if (
      droppedGroup?.mode !== "ordered" ||
      droppedGroup.steps[1].kind !== "graphql"
    ) throw new Error("missing frozen GraphQL suffix step")
    droppedGroup.steps[1].operation = { document: "mutation { __typename }" }
    droppedGroup.steps[1].response = {
      kind: "data",
      data: { __typename: "Mutation" },
    }
    await write(zero, droppedMutation)
    await assertRejects(load, SchemaError, "effect-free query steps")

    const asset = JSON.parse(
      await Deno.readTextFile(
        new URL(
          "./f02b-fixed-host-cases/f02b-control-direct-egress.json",
          import.meta.url,
        ),
      ),
    )
    await write(zero, { ...asset, id: "sample", route: "linear" })
    await assertRejects(load, SchemaError, "effect-free query steps")

    const lanes = structuredClone(two)
    const ordered = lanes.graphql?.groups[0]
    if (lanes.graphql == null || ordered?.mode !== "ordered") {
      throw new Error("missing frozen ordered group")
    }
    lanes.graphql.groups = [{
      mode: "lanes",
      timeoutMs: 1000,
      lanes: [
        { id: "first", steps: [ordered.steps[0]] },
        { id: "second", steps: [ordered.steps[1]] },
      ],
    }]
    await write(zero, lanes)
    await assertRejects(load, SchemaError, "effect-free ordered group")

    const changedRecords = structuredClone(one)
    if (changedRecords.graphql == null) throw new Error("missing fixture")
    changedRecords.graphql.expectedRecords = { unexpected: true }
    await write(zero, changedRecords)
    await assertRejects(load, SchemaError, "effect-free ordered group")

    await write(zero, validCase())
    await assertRejects(load, SchemaError, "needs a GraphQL fixture")

    await write(golden({ graphql: { steps: [] } }, ["graphql-fixture"]), one)
    await assertRejects(load, SchemaError, "requires the Rust User-Agent")

    await write(graphqlGolden([], ["graphql-user-agent"]), one)
    await assertRejects(load, SchemaError, "approvedSurfaces")
    await write(graphqlGolden([], ["graphql-fixture"]), one)
    await assertRejects(load, SchemaError, "approvedSurfaces")

    await write(
      golden({
        graphql: { steps: [], extra: true },
        graphqlUserAgent: USER_AGENT,
      }, ["graphql-fixture", "graphql-user-agent"]),
      one,
    )
    await assertRejects(load, SchemaError, "candidate.graphql")

    await write(zero, one)
    await load()
    const casePath = join(dir, "sample.json")
    const pinned = JSON.parse(await Deno.readTextFile(casePath))
    pinned.deviation.sha256 = "0".repeat(64)
    await Deno.writeTextFile(casePath, JSON.stringify(pinned))
    await assertRejects(load, SchemaError, "SHA-256 differs")
  })
})

Deno.test("zero-request fixture schema is candidate-only and exact", async () => {
  const one = parseCase(await frozenGraphQLCase("c016-default-team"))
  const fixture = one.graphql
  assert(fixture != null)
  const zero: GraphQLFixtureSpec = {
    ...fixture,
    expectedRequests: 0,
    groups: [],
  }
  assert(v.safeParse(ZeroRequestCandidateGraphQLSchema, zero).success)
  // A frozen case can never declare zero requests directly.
  assertThrows(() => parseCase({ ...one, graphql: zero }), SchemaError)
  assert(!v.safeParse(GraphQLFixtureSchema, zero).success)
  const emptyOrdered: GraphQLFixtureSpec = {
    ...zero,
    groups: [{ mode: "ordered", steps: [] }],
  }
  const extraField = { ...zero, extra: true }
  for (
    const invalid of [
      { ...zero, groups: fixture.groups },
      { ...zero, expectedRequests: 1 },
      emptyOrdered,
      extraField,
    ]
  ) {
    assert(!v.safeParse(ZeroRequestCandidateGraphQLSchema, invalid).success)
  }
  // resolveCase applies the zero schema only with a User-Agent and empty
  // groups; every other shape takes the strict frozen schema.
  const withFixture = (graphql: GraphQLFixtureSpec): CaseSpec => ({
    ...one,
    graphql,
  })
  assertEquals(
    resolveCase(withFixture(zero), DUMMY_VALUES, USER_AGENT).graphql?.groups,
    [],
  )
  assertThrows(() => resolveCase(withFixture(zero), DUMMY_VALUES))
  for (
    const invalid of [
      { ...zero, expectedRequests: 1 },
      emptyOrdered,
      extraField,
    ]
  ) {
    assertThrows(() =>
      resolveCase(withFixture(invalid), DUMMY_VALUES, USER_AGENT)
    )
  }
})

Deno.test("zero-request candidate passes only when no request reaches the fixture server", async () => {
  await withCorpus(async (dir, write) => {
    const spec = await frozenGraphQLCase("c016-default-team")
    const group = parseCase(spec).graphql?.groups[0]
    if (group?.mode !== "ordered" || group.steps[0].kind !== "graphql") {
      throw new Error("expected one frozen GraphQL step")
    }
    const step = group.steps[0]
    const body = JSON.stringify({
      query: step.operation.document,
      variables: step.operation.variables,
    })
    spec.expected = {
      exit: { code: 0 },
      stdout: { utf8: "ok" },
      stderr: { utf8: "" },
      fileEffects: [],
    }
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-graphql-zero-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement: await prepareConfinement({
          denoDir,
          statusHelper: await testStatusHelper(runDir),
        }),
        sandboxParent: runDir,
      }
      const script = async (
        name: string,
        requests: Array<{ userAgent: string; url: string }>,
      ): Promise<Program> => {
        const path = join(runDir, name)
        const lines = requests.map(({ userAgent, url }) =>
          `/usr/bin/curl --silent --show-error --noproxy '*' --request POST --header 'content-type: application/json' --header 'authorization: lin_api_fake' --header 'user-agent: ${userAgent}' --data-raw '${body}' "${url}" >/dev/null`
        )
        await Deno.writeTextFile(
          path,
          `#!/bin/sh\n${lines.join("\n")}\nprintf ok\n`,
          { mode: 0o755 },
        )
        return { kind: "executable", path }
      }
      const endpoint = "$LINEAR_GRAPHQL_ENDPOINT"
      const baseline = await script("baseline.sh", [{
        userAgent: "schpet-linear-cli/2.6.0",
        url: endpoint,
      }])
      const silentBaseline = await script("silent-baseline.sh", [])
      const zero = await script("zero.sh", [])
      const request = await script("request.sh", [{
        userAgent: USER_AGENT,
        url: endpoint,
      }])
      const wrongPath = await script("wrong-path.sh", [{
        userAgent: USER_AGENT,
        url: "${LINEAR_GRAPHQL_ENDPOINT%/graphql}/elsewhere",
      }])
      const selected = new Set(["linear"])
      await write(graphqlGolden([]), spec)
      const [loaded] = await loadCases(dir, selected, undefined, CONTRACT)
      const frozen = structuredClone(loaded.spec)
      const run = async (base: Program, program: Program) => {
        const [result] = await runCorpus([loaded], base, {
          name: "synthetic zero-request Rust candidate",
          contract: CONTRACT,
          program,
          implementedRoutes: selected,
        }, ctx)
        return result
      }

      const pass = await run(baseline, zero)
      assertEquals(pass.baseline.mismatches, [])
      assertEquals(pass.baseline.fixture?.graphqlRequests, 1)
      assertEquals(pass.baseline.fixture?.userAgents, [
        "schpet-linear-cli/2.6.0",
      ])
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.candidate?.fixture, {
        requests: 0,
        unexpected: 0,
        authorizationMatched: [],
        userAgents: [],
        graphqlRequests: 0,
        assetRequests: 0,
      })
      assertEquals(pass.reviewedDeviation?.approvedSurfaces, [
        "graphql-fixture",
        "graphql-user-agent",
      ])
      assertEquals(countReviewedDeviationPasses([pass]), 1)
      assertEquals(countReviewedGraphqlUserAgentPasses([pass]), 0)

      for (const program of [request, wrongPath]) {
        const failed = await run(baseline, program)
        assertEquals(failed.baseline.mismatches, [])
        assertEquals(failed.status, "fail")
        assertEquals(failed.candidate?.fixture?.requests, 1)
        assertEquals(failed.candidate?.fixture?.unexpected, 1)
        assertEquals(
          failed.candidate?.mismatches.map((mismatch) => mismatch.surface),
          ["fixture", "fixture"],
        )
        assertEquals(
          failed.candidate?.mismatches[0].detail,
          "unexpected request after final interaction",
        )
      }

      const drift = await run(silentBaseline, zero)
      assertEquals(drift.status, "baseline-drift")
      assertEquals(drift.candidate, null)
      assert(
        drift.baseline.mismatches.some((mismatch) =>
          mismatch.surface === "fixture"
        ),
      )
      assertEquals(loaded.spec, frozen)
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})

Deno.test("golden format v1 is closed and complete", () => {
  const valid = golden({ argv: ["--v3"] }, ["argv"])
  assertEquals(parseReviewedGolden(valid).formatVersion, 1)
  for (
    const mutation of [
      { ...valid, formatVersion: 2 },
      { ...valid, caseId: "../escape" },
      { ...valid, contract: "rust-next" },
      { ...valid, extra: 1 },
      { ...valid, candidate: { argv: ["--v3"], env: { X: "Y" } } },
      { ...valid, candidate: { timeoutMs: 1 } },
      { ...valid, candidate: { outputCapBytes: 1 } },
      { ...valid, candidate: { expected: { stdout: { utf8: "x" } } } },
      { ...valid, approvedSurfaces: ["argv", "argv"] },
      { ...valid, approvedSurfaces: ["wildcard"] },
      { ...valid, candidate: { graphqlUserAgent: "schpet-linear-cli/3.0.0" } },
    ]
  ) {
    assertThrows(() => parseReviewedGolden(mutation), SchemaError)
  }
  const spec = validCase()
  spec.deviation = { id: GOLDEN_ID, contract: CONTRACT, sha256: "a".repeat(64) }
  assertEquals(parseCase(spec).deviation?.id, GOLDEN_ID)
})

Deno.test("nested cwd root is allowed only in reviewed expected output", async () => {
  await withCorpus(async (dir, write) => {
    const spec = validCase()
    spec.gitProbe = "parent-root"
    spec.cwdSubdir = "subdir"
    const substitutions = spec.substitutions
    if (!Array.isArray(substitutions)) throw new Error("missing substitutions")
    substitutions.push("cwd")
    substitutions.push("cwdRoot")
    const expected = parseCase(spec).expected
    await write(
      golden({
        expected: {
          ...expected,
          stderr: { utf8: "root={{cwdRoot}} invoked={{cwd}}" },
        },
      }, ["stderr"]),
      spec,
    )
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    assertEquals(
      candidateCaseView(loaded).spec.expected.stderr,
      { utf8: "root={{cwdRoot}} invoked={{cwd}}" },
    )
    await write(golden({ argv: ["{{cwdRoot}}"] }, ["argv"]), spec)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "cwdRoot is restricted to expected output",
    )
    await write(
      golden({
        expected: {
          ...expected,
          fileEffects: [{
            path: "link",
            change: "created",
            kind: "symlink",
            target: "{{cwdRoot}}",
          }],
        },
      }, ["files"]),
      spec,
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "cwdRoot is restricted to expected output",
    )
  })
})

Deno.test("corpus validates SHA, identity, surfaces and orphan files before filtering", async () => {
  await withCorpus(async (dir, write) => {
    const routes = new Set(["linear"])
    const good = golden({ argv: ["--v3"] }, ["argv"])
    await write(good)
    const [loaded] = await loadCases(dir, routes, undefined, CONTRACT)
    assertEquals(loaded.golden?.spec.caseId, "sample")
    assertEquals(candidateCaseView(loaded).spec.argv, ["--v3"])
    assertEquals(loaded.spec.argv, ["--version"])
    await assertProposalOutsideGoldens(join(dir, "proposals"), dir)
    await assertProposalOutsideGoldens(join(dir, "sibling"), dir)
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "rust-goldens", CONTRACT, "new"),
          dir,
        ),
      Error,
      "outside",
    )
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "rust-goldens", "..x"),
          dir,
        ),
      Error,
      "outside",
    )
    await Deno.symlink(join(dir, "rust-goldens"), join(dir, "linked-goldens"))
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "linked-goldens", "future"),
          dir,
        ),
      Error,
      "outside",
    )

    const path = join(dir, "rust-goldens", CONTRACT, "sample.json")
    const root = join(dir, "rust-goldens")
    const contractRoot = join(root, CONTRACT)
    const writeRaw = async (bytes: Uint8Array) => {
      await Deno.writeFile(path, bytes)
      const spec = validCase()
      spec.deviation = {
        id: GOLDEN_ID,
        contract: CONTRACT,
        sha256: await sha256Hex(bytes),
      }
      await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    }
    await Deno.remove(path)
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "missing or unsafe",
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "missing or unsafe",
    )
    await writeRaw(new TextEncoder().encode("{"))
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "not UTF-8 JSON",
    )
    await writeRaw(new Uint8Array([0xff]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "not UTF-8 JSON",
    )
    await write(good)
    await Deno.writeTextFile(path, JSON.stringify({ ...good, caseId: "wrong" }))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "SHA-256",
    )
    await write({ ...good, caseId: "wrong" })
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "identity",
    )
    await write(golden({ argv: ["--version"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "redundant",
    )
    await write(golden({ argv: ["--v3"] }, ["stdout"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "approvedSurfaces",
    )
    await write(golden({ argv: ["{{fixturePort}}"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "not declared",
    )
    const baseExpected = parseCase(validCase()).expected
    await write(golden({
      expected: {
        ...baseExpected,
        stderr: { utf8: "{{referenceModuleUrl}}/src/credentials.ts" },
      },
    }, ["stderr"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "Rust golden cannot use referenceModuleUrl",
    )
    await write(golden({ argv: ["{{referenceModuleUrl}}"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "Rust golden cannot use referenceModuleUrl",
    )
    await write(golden({
      expected: { ...baseExpected, stdout: { base64: "eA==" } },
    }, ["stdout"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "redundant",
    )
    await write(
      golden({
        expected: {
          ...baseExpected,
          stdout: {
            mode: "close-after-bytes",
            count: 2,
            prefix: { utf8: "ab" },
          },
        },
      }, ["stdout"]),
      { ...validCase(), outputCapBytes: 1 },
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "exceeds outputCapBytes",
    )
    await write(good)
    await Deno.writeTextFile(
      join(dir, "rust-goldens", CONTRACT, "orphan.json"),
      "{}",
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "orphan",
    )
    await Deno.remove(join(dir, "rust-goldens", CONTRACT, "orphan.json"))
    await Deno.writeTextFile(join(contractRoot, "notes.txt"), "not a golden")
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "unsafe reviewed golden entry",
    )
    await Deno.remove(join(contractRoot, "notes.txt"))
    await Deno.mkdir(join(root, "rust-next"))
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "unknown or unsafe reviewed golden contract",
    )
    await Deno.remove(join(root, "rust-next"))
    await Deno.rename(contractRoot, join(root, "contract-real"))
    await Deno.symlink(join(root, "contract-real"), contractRoot)
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "missing or unsafe",
    )
    await Deno.remove(contractRoot)
    await Deno.rename(join(root, "contract-real"), contractRoot)
    await Deno.rename(root, join(dir, "goldens-real"))
    await Deno.symlink(join(dir, "goldens-real"), root)
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "missing or unsafe",
    )
    await Deno.remove(root)
    await Deno.rename(join(dir, "goldens-real"), root)
    await Deno.remove(path)
    await Deno.symlink(join(dir, "sample.json"), path)
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "unsafe",
    )
  })
})

Deno.test("all committed GraphQL cases bind exact Rust User-Agent without changing frozen fixtures", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set<string>(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest route has no path")
    }
    return route.path
  }))
  const cases = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
    undefined,
    CONTRACT,
  )
  // C002 binds its own row-text goldens in c002-main-cases.test.ts.
  const graphql = cases.filter((loaded) =>
    loaded.spec.graphql != null && !loaded.spec.id.startsWith("c002-")
  )
  const inventory = await caseDirectoryInventory(
    new URL("./cases", import.meta.url).pathname,
    routes,
  )
  assertCaseCoverage(cases, inventory)
  assertSameIds(
    graphql.map((entry) => entry.spec.id),
    [...inventory.values()].filter((entry) =>
      entry.kind === "graphql" && !entry.id.startsWith("c002-")
    ).map((entry) => entry.id),
    "reviewed GraphQL directory coverage",
  )
  const c011Surfaces = new Map<string, string[]>([
    ["c011-infinite-position", ["stderr", "graphql-user-agent"]],
    ["c011-null-position-pair", ["stderr", "graphql-user-agent"]],
    ["c011-null-position-single", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c011-null-states", ["stderr", "graphql-user-agent"]],
    ["c011-null-team", ["stderr", "graphql-user-agent"]],
    ["c011-string-position", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c011-raw-extra-field", ["stdout", "graphql-user-agent"]],
    ["c011-width-u4dc0", ["stdout", "graphql-user-agent"]],
  ])
  const c021Surfaces = new Map<string, string[]>([
    ["c021-hexagram-width-text", ["stdout", "graphql-user-agent"]],
    ["c021-http-500", ["stderr", "graphql-user-agent"]],
    ["c021-raw-extra-field", ["stdout", "graphql-user-agent"]],
    ["c021-raw-lone-surrogate", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c021-raw-missing-null", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c021-raw-template-data-object", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c021-transport-error", ["stderr", "graphql-user-agent"]],
  ])
  const c022Graphql = new Map<string, [string, string[]]>([
    ["c022-json-extra-field", ["C022-TYPED-JSON-FIELDS", [
      "stdout",
      "graphql-user-agent",
    ]]],
    ["c022-json-missing-name", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-json-null-outer", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-json-null-template", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-json-object-outer", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-lone-surrogate", ["C022-INNER-LONE-SURROGATE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-markdown-escapes", ["C022-TEMPLATE-BODY-MARKDOWN", [
      "stdout",
      "graphql-user-agent",
    ]]],
    ["c022-non-json-response", ["C022-TRANSPORT-DIAGNOSTIC", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-number-infinity", ["C022-INNER-NONFINITE-NUMBER", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-text-missing-name", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-text-null-outer", ["C022-STRICT-TEMPLATE-DECODE", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-text-null-template", ["C022-STRICT-TEMPLATE-DECODE", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c022-text-object-outer", ["C022-STRICT-TEMPLATE-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
  ])
  assertEquals(c022Graphql.size, 13)
  const c010Graphql = new Map<string, [string, string[]]>([
    ["c010-extra-field", ["C010-TYPED-JSON-FIELDS", [
      "stdout",
      "graphql-user-agent",
    ]]],
    ["c010-http-error", ["C010-TRANSPORT-DIAGNOSTIC", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c010-null-display", ["C010-STRICT-MEMBER-DECODE", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c010-null-team", ["C010-STRICT-MEMBER-DECODE", [
      "stderr",
      "graphql-user-agent",
    ]]],
    ["c010-wrong-type", ["C010-STRICT-MEMBER-DECODE", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]]],
  ])
  assertEquals(c010Graphql.size, 5)
  const c016Surfaces = new Map<string, string[]>([
    ["c016-combined-workspace-only", [
      "argv",
      "graphql-user-agent",
    ]],
    ["c016-collision-project-key", ["argv", "graphql-user-agent"]],
    ["c016-connection-null", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c016-cursor-null", [
      "exit",
      "stdout",
      "stderr",
      "graphql-fixture",
      "graphql-user-agent",
    ]],
    ["c016-cursor-repeat", [
      "exit",
      "stdout",
      "stderr",
      "graphql-fixture",
      "graphql-user-agent",
    ]],
    ["c016-http-503", ["stderr", "graphql-user-agent"]],
    ["c016-nodes-malformed", ["stderr", "graphql-user-agent"]],
    ["c016-page-info-missing", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
    ]],
    ["c016-root-workspace-bad-team", [
      "argv",
      "graphql-fixture",
      "graphql-user-agent",
    ]],
    ["c016-root-workspace-success", ["graphql-fixture", "graphql-user-agent"]],
    ["c016-team-malformed", ["stderr", "graphql-user-agent"]],
    ["c016-team-vs-workspace", ["argv", "graphql-user-agent"]],
  ])
  const c016FixtureDeltas = new Set([
    "c016-cursor-null",
    "c016-cursor-repeat",
    "c016-root-workspace-bad-team",
    "c016-root-workspace-success",
  ])
  const c023FixtureDeltas = new Set([
    "c023-missing-cursor",
    "c023-null-cursor",
    "c023f-repeat-cursor",
  ])
  const c020FixtureDeltas = new Set(["c020-cycles-null-later"])
  const c030FixtureDeltas = new Set([
    "c030-name-lookup-null-projects",
    "c030-repeat-cursor-finite",
    "c030-repeat-cursor-finite-text",
  ])
  const c031FixtureDeltas = new Set([
    "c031-all-repeat-cursor-finite",
    "c031-bare-linear-url-legacy",
    "c031-bare-linear-unsupported-legacy",
  ])
  const c027FixtureDeltas = new Set(["c027-json-equals-empty"])
  const c035FixtureDeltas = new Set(["c035-json-equals-empty"])
  const c038FixtureDeltas = new Set([
    "c038-empty-id",
    "c038-slug-and-name-errors",
    "c038-slug-http-401-name-hit",
  ])
  const c046Surfaces = new Map<string, string[]>([
    ["c046-strict-nonenvelope", [
      "stderr",
      "graphql-user-agent",
      "graphql-fixture",
    ]],
    ...[
      "name-error-miss",
      "url-miss",
      "details-empty",
      "non-tty-confirmation",
      "strict-null-details",
    ].map((suffix): [string, string[]] => [
      `c046-${suffix}`,
      ["stderr", "graphql-user-agent"],
    ]),
    ["c046-strict-later-node", [
      "exit",
      "stdout",
      "stderr",
      "graphql-user-agent",
      "graphql-fixture",
    ]],
  ])
  const c018Surfaces = new Map<string, string[]>([
    ["c018-strict-null-node", ["stderr", "graphql-user-agent"]],
    ["c018-strict-null-uuid", [
      "stderr",
      "graphql-user-agent",
      "graphql-fixture",
    ]],
  ])
  const c018FixtureDeltas = new Set(["c018-strict-null-uuid"])
  // Each comment-add caller retains none of its one frozen source query.
  const commentAddFixtureDeltas = new Set([
    "c028-url-truncated-utf8",
    "c044-url-surrogate-utf8",
    "c055-null-content-overlong-utf8",
  ])
  const c046FixtureDeltas = new Set([
    "c046-strict-later-node",
    "c046-strict-nonenvelope",
  ])
  const substitutions = {
    home: "h",
    configHome: "c",
    cwd: "w",
    cwdRoot: "r",
    bin: "b",
    denoDir: "d",
    fixturePort: "1234",
    referenceModuleUrl: "file:///reference",
  }
  for (const loaded of graphql) {
    // This closed SHA-bound native parser cohort has its own guard. Keep
    // source fixtures intact and explicitly assert candidate requests/effects.
    const native = nativeParserContract(loaded.spec.id)
    if (native != null) {
      assertEquals(loaded.golden?.spec.deviationId, native[0])
      assertEquals(loaded.golden?.spec.approvedSurfaces, native[1])
      const candidate = candidateCaseView(loaded)
      if (loaded.spec.graphql != null) {
        assertEquals(loaded.golden?.spec.candidate.graphqlUserAgent, USER_AGENT)
        assertEquals(candidate.spec.graphql?.expectedRequests, 0)
        assertEquals(candidate.spec.graphql?.groups, [])
        assertEquals(
          candidate.spec.graphql?.expectedRecords,
          loaded.spec.graphql.initialRecords,
        )
      }
      continue
    }

    assertEquals(
      (loaded.golden ?? loaded.goldenV2)?.spec.candidate.graphqlUserAgent,
      USER_AGENT,
    )
    if (loaded.spec.id.startsWith("c022-")) {
      assertEquals(
        loaded.spec.deviation?.id,
        c022Graphql.get(loaded.spec.id)?.[0] ?? "R01H-GRAPHQL-UA",
        loaded.spec.id,
      )
    }
    if (loaded.spec.id.startsWith("c010-")) {
      assertEquals(
        loaded.spec.deviation?.id,
        c010Graphql.get(loaded.spec.id)?.[0] ?? "R01H-GRAPHQL-UA",
        loaded.spec.id,
      )
    }
    if (loaded.spec.id.startsWith("f06e0-")) {
      assertEquals(loaded.spec.deviation?.id, "R01H-GRAPHQL-UA", loaded.spec.id)
    }
    // These cohorts pin their approved surfaces in their own corpus guards.
    if (
      !loaded.spec.id.startsWith("c023") &&
      !loaded.spec.id.startsWith("c024-") &&
      !loaded.spec.id.startsWith("c020-") &&
      !loaded.spec.id.startsWith("c030-") &&
      !loaded.spec.id.startsWith("c031-") &&
      !loaded.spec.id.startsWith("c027-") &&
      !loaded.spec.id.startsWith("c035-") &&
      !loaded.spec.id.startsWith("c037-") &&
      !/^c0(38|39|48|43|54|32|33|74|29|34|58|59|17|28|44|55|66|77|78|76|64|65|41|42|45|47|63)-/
        .test(
          loaded.spec.id,
        )
    ) {
      assertEquals(
        loaded.golden?.spec.approvedSurfaces,
        c011Surfaces.get(loaded.spec.id) ??
          c010Graphql.get(loaded.spec.id)?.[1] ??
          c021Surfaces.get(loaded.spec.id) ??
          c022Graphql.get(loaded.spec.id)?.[1] ??
          c016Surfaces.get(loaded.spec.id) ??
          c046Surfaces.get(loaded.spec.id) ??
          c018Surfaces.get(loaded.spec.id) ??
          (loaded.spec.id === "c019-transport-error"
            ? ["stderr", "graphql-user-agent"]
            : undefined) ??
          (loaded.spec.id === "c008-text-percent" ||
              loaded.spec.id === "c008-raw-extra-field" ||
              loaded.spec.id === "c015-raw-extra-json"
            ? ["stdout", "graphql-user-agent"]
            : loaded.spec.id === "c008-raw-null-name"
            ? ["exit", "stdout", "stderr", "graphql-user-agent"]
            : ["graphql-user-agent"]),
      )
    }
    if (loaded.spec.id.startsWith("c037-")) {
      // C037's four v2 cases append one or two pages (2-3 requests total).
      // Its guard pins exact surfaces, no argv, and the sole allowed empty
      // v1 GraphQL delta; P02's focused tests verify page projection.
      continue
    }
    const frozen = resolveCase(loaded.spec, substitutions).graphql
    const candidate = resolveCase(
      candidateCaseView(loaded).spec,
      substitutions,
      candidateCaseView(loaded).runtimeUserAgent,
    ).graphql
    assert(frozen != null && candidate != null)
    const expected = structuredClone(frozen)
    for (const group of expected.groups) {
      const steps = group.mode === "ordered"
        ? group.steps
        : group.lanes.flatMap((lane) => lane.steps)
      for (const step of steps) {
        if (step.kind === "graphql") {
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          step.identity.userAgent = USER_AGENT
        }
      }
    }
    if (
      c016FixtureDeltas.has(loaded.spec.id) ||
      c023FixtureDeltas.has(loaded.spec.id) ||
      c020FixtureDeltas.has(loaded.spec.id) ||
      c030FixtureDeltas.has(loaded.spec.id) ||
      c031FixtureDeltas.has(loaded.spec.id) ||
      c027FixtureDeltas.has(loaded.spec.id) ||
      c035FixtureDeltas.has(loaded.spec.id) ||
      c038FixtureDeltas.has(loaded.spec.id) ||
      c046FixtureDeltas.has(loaded.spec.id) ||
      c018FixtureDeltas.has(loaded.spec.id) ||
      commentAddFixtureDeltas.has(loaded.spec.id)
    ) {
      const delta = loaded.golden?.spec.candidate.graphql
      const group = expected.groups[0]
      if (delta == null || group?.mode !== "ordered") {
        throw new Error(`${loaded.spec.id}: missing reviewed ordered delta`)
      }
      group.steps = group.steps.slice(0, delta.steps.length)
      for (const [index, override] of delta.steps.entries()) {
        const step = group.steps[index]
        if (step.kind !== "graphql" || step.id !== override.id) {
          throw new Error(`${loaded.spec.id}: unexpected reviewed step`)
        }
        if (override.variables != null) {
          step.operation.variables = override.variables
        }
      }
      expected.expectedRequests = group.steps.length
      if (
        (c031FixtureDeltas.has(loaded.spec.id) ||
          c027FixtureDeltas.has(loaded.spec.id) ||
          c035FixtureDeltas.has(loaded.spec.id) ||
          c038FixtureDeltas.has(loaded.spec.id) ||
          commentAddFixtureDeltas.has(loaded.spec.id)) &&
        delta.steps.length === 0
      ) {
        expected.groups = []
      }
    }
    assertEquals(candidate, expected, loaded.spec.id)
  }
  const frozen = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
    "api-graphql-viewer",
  )
  assertEquals(frozen.length, 1)
  assertEquals(
    frozen[0].spec.argv,
    cases.find((loaded) => loaded.spec.id === "api-graphql-viewer")?.spec.argv,
  )
})

Deno.test("Rust contract requires an executable descriptor", async () => {
  const dir = await Deno.makeTempDir({ prefix: "linear-rust-contract-" })
  try {
    const path = join(dir, "candidate.json")
    await Deno.writeTextFile(
      path,
      JSON.stringify({
        name: "interpreted",
        contract: CONTRACT,
        program: { kind: "interpreted-reference", workspace: dir },
        implementedRoutes: ["linear"],
      }),
    )
    const options = parseOptions([
      "--reference",
      "/reference",
      "--reference-binary",
      "/binary",
      "--candidate",
      path,
    ])
    await assertRejects(
      () => loadCandidate(options, new Set(["linear"])),
      Error,
      "requires an executable",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("legacy fixtureServer cases cannot claim a GraphQL User-Agent override", async () => {
  await withCorpus(async (dir, write) => {
    const spec = validCase()
    spec.substitutions = ["home", "configHome", "bin", "denoDir", "fixturePort"]
    spec.fixtureServer = {
      path: "/graphql",
      responses: [{ status: 200, headers: {}, body: { utf8: "ok" } }],
      expectedRequests: 1,
      expectedAuthorization: "lin_api_fake",
    }
    await write(
      golden({ graphqlUserAgent: USER_AGENT }, ["graphql-user-agent"]),
      spec,
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "requires a GraphQL fixture",
    )
  })
})

Deno.test("reviewed argv and byte differences use separate exact candidate view", async () => {
  await withCorpus(async (dir, write) => {
    const baseExpected = parseCase(validCase()).expected
    await write(golden({
      argv: ["--v3"],
      expected: { ...baseExpected, stdout: { utf8: "y" } },
    }, ["argv", "stdout"]))
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-run-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const confinement = await prepareConfinement({
        denoDir,
        statusHelper: await testStatusHelper(runDir),
      })
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement,
        sandboxParent: runDir,
      }
      const good = join(runDir, "good.sh")
      await Deno.writeTextFile(
        good,
        '#!/bin/sh\nif [ "$1" = "--v3" ]; then printf y; else printf x; fi\n',
        { mode: 0o755 },
      )
      const bad = join(runDir, "bad.sh")
      await Deno.writeTextFile(bad, "#!/bin/sh\nprintf z\n", { mode: 0o755 })
      const baseline: Program = { kind: "executable", path: good }
      const selected = new Set(["linear"])
      const [pass] = await runCorpus([loaded], baseline, {
        name: "rust",
        contract: CONTRACT,
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(pass.status, "pass")
      assertEquals(pass.baseline.observation.stdoutBytes, 1)
      assertEquals(pass.candidate?.observation.stdoutBytes, 1)
      assertEquals(pass.reviewedDeviation?.approvedSurfaces, ["argv", "stdout"])
      assertEquals(countReviewedDeviationPasses([pass]), 1)
      assertEquals(countReviewedGraphqlUserAgentPasses([pass]), 0)
      assert(pass.reviewedDeviation != null)
      assertEquals(
        countReviewedGraphqlUserAgentPasses([{
          ...pass,
          reviewedDeviation: {
            ...pass.reviewedDeviation,
            approvedSurfaces: ["graphql-user-agent"],
          },
        }]),
        1,
      )
      assertEquals(
        toReportCase(pass).reviewedDeviation?.sha256,
        loaded.golden?.sha256,
      )

      const [frozen] = await runCorpus([loaded], baseline, {
        name: "frozen",
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(frozen.status, "pass")
      assertEquals(frozen.reviewedDeviation, null)

      const [failed] = await runCorpus([loaded], baseline, {
        name: "bad",
        contract: CONTRACT,
        program: { kind: "executable", path: bad },
        implementedRoutes: selected,
      }, ctx)
      assertEquals(failed.status, "fail")
      assertEquals(
        failed.candidate?.mismatches.map((mismatch) => mismatch.surface),
        ["stdout"],
      )
      assertEquals(countReviewedDeviationPasses([failed]), 0)

      for (
        const variant of [
          {
            name: "stderr",
            source: "#!/bin/sh\nprintf y\nprintf z >&2\n",
            surface: "stderr",
          },
          {
            name: "exit-code",
            source: "#!/bin/sh\nprintf y\nexit 3\n",
            surface: "exit",
          },
          {
            name: "file-effect",
            source: '#!/bin/sh\nprintf y\nprintf z > "$HOME/extra"\n',
            surface: "files",
          },
        ]
      ) {
        const executable = join(runDir, `${variant.name}.sh`)
        await Deno.writeTextFile(executable, variant.source, { mode: 0o755 })
        const [result] = await runCorpus([loaded], baseline, {
          name: variant.name,
          contract: CONTRACT,
          program: { kind: "executable", path: executable },
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.status, "fail", variant.name)
        assertEquals(
          result.candidate?.mismatches.map((mismatch) => mismatch.surface),
          [variant.surface],
          variant.name,
        )
      }

      await write(golden({
        argv: ["--v3"],
        expected: {
          ...baseExpected,
          stdout: { utf8: "y" },
          exit: { signal: "SIGTERM" },
        },
      }, ["argv", "exit", "stdout"]))
      const [signalGolden] = await loadCases(dir, selected, undefined, CONTRACT)
      const exited = join(runDir, "exit-143.sh")
      await Deno.writeTextFile(exited, "#!/bin/sh\nprintf y\nexit 143\n", {
        mode: 0o755,
      })
      const [codeVsSignal] = await runCorpus([signalGolden], baseline, {
        name: "code versus signal",
        contract: CONTRACT,
        program: { kind: "executable", path: exited },
        implementedRoutes: selected,
      }, ctx)
      assertEquals(codeVsSignal.status, "fail")
      assertEquals(
        codeVsSignal.candidate?.mismatches.map((mismatch) => mismatch.surface),
        ["exit"],
      )

      const [unimplemented] = await runCorpus([loaded], baseline, {
        name: "missing",
        contract: CONTRACT,
        program: { kind: "executable", path: bad },
        implementedRoutes: new Set(),
      }, ctx)
      assertEquals(unimplemented.status, "not-implemented")
      assertEquals(unimplemented.candidate, null)

      const [drift] = await runCorpus([loaded], {
        kind: "executable",
        path: bad,
      }, {
        name: "rust",
        contract: CONTRACT,
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(drift.status, "baseline-drift")
      assertEquals(drift.candidate, null)
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})

Deno.test("GraphQL candidate fixture requires exact v3 identity and preserves request matching", async () => {
  await withCorpus(async (dir, write) => {
    const source = JSON.parse(
      await Deno.readTextFile(
        new URL("./cases/api-graphql-viewer.json", import.meta.url),
      ),
    )
    const request = '{"query":"{ viewer { id } }"}'
    const argv = (
      userAgent: string,
      authorization = "lin_api_fake",
      body = request,
    ) => [
      "--silent",
      "--show-error",
      "--noproxy",
      "*",
      "--request",
      "POST",
      "--header",
      "content-type: application/json",
      "--header",
      `authorization: ${authorization}`,
      "--header",
      `user-agent: ${userAgent}`,
      "--data-raw",
      body,
      "http://127.0.0.1:{{fixturePort}}/graphql",
    ]
    const caseSpec = {
      ...source,
      id: "sample",
      route: "linear",
      argv: argv("schpet-linear-cli/2.6.0"),
    }
    await write(golden({ argv: argv(USER_AGENT) }, ["argv"]), caseSpec)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), "unselected", CONTRACT),
      SchemaError,
      "requires exact GraphQL User-Agent binding",
    )
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-graphql-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement: await prepareConfinement({
          denoDir,
          statusHelper: await testStatusHelper(runDir),
        }),
        sandboxParent: runDir,
      }
      const program: Program = { kind: "executable", path: "/usr/bin/curl" }
      const selected = new Set(["linear"])
      const run = async (candidateArgv: string[]) => {
        await write(
          golden({
            argv: candidateArgv,
            graphqlUserAgent: USER_AGENT,
          }, ["argv", "graphql-user-agent"]),
          caseSpec,
        )
        const [loaded] = await loadCases(dir, selected, undefined, CONTRACT)
        const [result] = await runCorpus([loaded], program, {
          name: "synthetic v3",
          contract: CONTRACT,
          program,
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.baseline.mismatches, [])
        assertEquals(result.baseline.fixture?.userAgents, [
          "schpet-linear-cli/2.6.0",
        ])
        return result
      }
      const pass = await run(argv(USER_AGENT))
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.candidate?.fixture?.userAgents, [USER_AGENT])
      assertEquals(pass.candidate?.fixture?.graphqlRequests, 1)

      for (
        const invalid of [
          argv("schpet-linear-cli/3.0.0"),
          argv(USER_AGENT, "lin_api_fake_wrong"),
          argv(
            USER_AGENT,
            "lin_api_fake",
            '{"query":"{ viewer { id } }","variables":{"x":1}}',
          ),
          [...argv(USER_AGENT), "http://127.0.0.1:{{fixturePort}}/graphql"],
        ]
      ) {
        const failed = await run(invalid)
        assertEquals(failed.status, "fail")
        assert(
          failed.candidate?.mismatches.some((mismatch) =>
            mismatch.surface === "fixture"
          ),
        )
      }
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})
