// Case corpus loading: schema validation, manifest route binding, fixture
// existence, and placeholder resolution into concrete bytes and paths.
import { join, relative } from "@std/path"
import {
  getNamedType,
  getOperationAST,
  type GraphQLSchema,
  type GraphQLType,
  isInterfaceType,
  isListType,
  isNonNullType,
  isObjectType,
  isUnionType,
  Kind,
  parse,
  print,
} from "graphql"
import { bytesEqual, decodeByteValue, sha256Hex } from "./bytes.ts"
import type { StdoutMode } from "./target-status.ts"
import { buildPinnedSchema, matchGraphQL } from "./graphql-match.ts"
import { projectGraphQLResponse } from "./graphql-server.ts"
import { GraphQLState } from "./graphql-state.ts"
import {
  type CandidateContract,
  type CaseSpec,
  FROZEN_CONTRACT,
  FROZEN_USER_AGENT,
  GraphQLFixtureSchema,
  type GraphQLStepSpec,
  type InteractionSpec,
  parseCase,
  parseReviewedGolden,
  parseReviewedGoldenV2,
  type ReviewedGolden,
  type ReviewedGoldenV2,
  type RuntimeGraphQLFixtureSpec,
  RUST_CONTRACT,
  RUST_USER_AGENT,
  SchemaError,
  substitute,
  type SubstitutionName,
  ZeroRequestCandidateGraphQLSchema,
} from "./schema.ts"
import * as v from "valibot"

export interface LoadedCase {
  file: string
  spec: CaseSpec
  /** Absolute fixture directory copied into the sandbox cwd, or null for empty. */
  fixtureDir: string | null
  configFixtureDir: string | null
  golden?: { spec: ReviewedGolden; sha256: string } | null
  goldenV2?: {
    spec: ReviewedGoldenV2
    sha256: string
    checkedFixture: NonNullable<CaseSpec["graphql"]>
  } | null
  /** Candidate-only override applied after the frozen case is resolved. */
  runtimeUserAgent?: typeof RUST_USER_AGENT
}

const checkedV2Cases = new WeakSet<LoadedCase>()

function same(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.length === right.length &&
      left.every((entry, index) => same(entry, right[index]))
  }
  if (record(left) && record(right)) {
    const keys = Object.keys(left)
    return keys.length === Object.keys(right).length &&
      keys.every((key) =>
        Object.hasOwn(right, key) && same(left[key], right[key])
      )
  }
  return false
}

function sameStdout(
  left: CaseSpec["expected"]["stdout"],
  right: CaseSpec["expected"]["stdout"],
): boolean {
  if ("mode" in left || "mode" in right) {
    if (!("mode" in left && "mode" in right)) return false
    if (left.mode !== right.mode) return false
    if (left.mode === "closed-at-start" && right.mode === "closed-at-start") {
      return true
    }
    if (
      left.mode === "close-after-bytes" && right.mode === "close-after-bytes"
    ) {
      return left.count === right.count && bytesEqual(
        decodeByteValue(left.prefix),
        decodeByteValue(right.prefix),
      )
    }
    return false
  }
  return bytesEqual(decodeByteValue(left), decodeByteValue(right))
}

/** Exact committed source fixture for the sole two-query zero-request exception. */
const C038_EMPTY_ID_FROZEN_GRAPHQL: NonNullable<CaseSpec["graphql"]> = {
  "path": "/graphql",
  "schemaSha256":
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  "expectedRequests": 2,
  "initialRecords": {},
  "expectedRecords": {},
  "groups": [
    {
      "mode": "ordered",
      "steps": [
        {
          "kind": "graphql",
          "id": "slug",
          "operation": {
            "document":
              "query GetInitiativeBySlugForView($slugId: String!) {\n      initiatives(filter: { slugId: { eq: $slugId } }) {\n        nodes {\n          id\n          slugId\n        }\n      }\n    }\n  ",
            "variables": {
              "slugId": "",
            },
          },
          "identity": {
            "authorization": "lin_api_fake_alpha",
            "userAgent": "schpet-linear-cli/2.6.0",
            "headers": {},
          },
          "response": {
            "kind": "data",
            "data": {
              "initiatives": {
                "nodes": [],
              },
            },
          },
          "effects": [],
        },
        {
          "kind": "graphql",
          "id": "name",
          "operation": {
            "document":
              "query GetInitiativeByNameForView($name: String!) {\n      initiatives(filter: { name: { eqIgnoreCase: $name } }) {\n        nodes {\n          id\n          name\n        }\n      }\n    }\n  ",
            "variables": {
              "name": "",
            },
          },
          "identity": {
            "authorization": "lin_api_fake_alpha",
            "userAgent": "schpet-linear-cli/2.6.0",
            "headers": {},
          },
          "response": {
            "kind": "data",
            "data": {
              "initiatives": {
                "nodes": [],
              },
            },
          },
          "effects": [],
        },
      ],
    },
  ],
}

function isC038EmptyIdSource(spec: CaseSpec): boolean {
  return spec.id === "c038-empty-id" &&
    spec.route === "linear initiative view" &&
    same(spec.argv, ["initiative", "view", ""]) &&
    spec.fixtureServer == null &&
    same(spec.graphql, C038_EMPTY_ID_FROZEN_GRAPHQL)
}

// The sole mutation exception: exact frozen C033 source input/effect state.
// A strict decimal parser rejects this input before any transport is prepared.
const C033_SORT_RADIX_FROZEN_GRAPHQL = {
  "path": "/graphql",
  "schemaSha256":
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  "expectedRequests": 1,
  "initialRecords": {
    "ProjectMilestone:00000000-0000-4000-9000-000000003301": {
      "id": "00000000-0000-4000-9000-000000003301",
      "name": "Existing",
      "description": "Keep description",
      "targetDate": "2026-10-01",
      "sortOrder": 5,
      "projectId": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
      "untouched": "preserve me",
    },
  },
  "expectedRecords": {
    "ProjectMilestone:00000000-0000-4000-9000-000000003301": {
      "id": "00000000-0000-4000-9000-000000003301",
      "name": "Existing",
      "description": "Keep description",
      "targetDate": "2026-10-01",
      "sortOrder": 16,
      "projectId": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
      "untouched": "preserve me",
    },
  },
  "groups": [
    {
      "mode": "ordered",
      "steps": [
        {
          "kind": "graphql",
          "id": "update",
          "operation": {
            "document":
              "mutation UpdateProjectMilestone($id: String!, $input: ProjectMilestoneUpdateInput!) {\n    projectMilestoneUpdate(id: $id, input: $input) {\n      success\n      projectMilestone {\n        id\n        name\n        targetDate\n        sortOrder\n        project {\n          id\n          name\n        }\n      }\n    }\n  }\n",
            "variables": {
              "id": "00000000-0000-4000-9000-000000003301",
              "input": {
                "sortOrder": 16,
              },
            },
          },
          "identity": {
            "authorization": "lin_api_fake",
            "userAgent": "schpet-linear-cli/2.6.0",
            "headers": {},
          },
          "response": {
            "kind": "data",
            "data": {
              "projectMilestoneUpdate": {
                "success": true,
                "projectMilestone": {
                  "id": "00000000-0000-4000-9000-000000003301",
                  "name": "Existing",
                  "targetDate": "2026-10-01",
                  "sortOrder": 16,
                  "project": {
                    "id": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
                    "name": "Mobile App",
                  },
                },
              },
            },
          },
          "effects": [
            {
              "kind": "put",
              "record": "ProjectMilestone:00000000-0000-4000-9000-000000003301",
              "before": {
                "value": {
                  "id": "00000000-0000-4000-9000-000000003301",
                  "name": "Existing",
                  "description": "Keep description",
                  "targetDate": "2026-10-01",
                  "sortOrder": 5,
                  "projectId": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
                  "untouched": "preserve me",
                },
              },
              "after": {
                "id": "00000000-0000-4000-9000-000000003301",
                "name": "Existing",
                "description": "Keep description",
                "targetDate": "2026-10-01",
                "sortOrder": 16,
                "projectId": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
                "untouched": "preserve me",
              },
            },
          ],
        },
      ],
    },
  ],
}

/** Derive only the reviewed candidate request script; never mutate the frozen fixture. */
function applyGraphQLDelta(
  spec: CaseSpec,
  delta: ReviewedGolden["candidate"]["graphql"],
  golden?: ReviewedGolden,
): CaseSpec["graphql"] {
  if (delta == null) return spec.graphql
  const fixture = spec.graphql
  if (fixture == null || spec.fixtureServer != null) {
    throw new SchemaError(
      `case ${spec.id}: GraphQL delta needs a GraphQL fixture`,
    )
  }
  if (spec.id === "c033-sort-radix" && delta.steps.length === 0) {
    const expected = golden?.candidate.expected
    if (
      spec.route !== "linear milestone update" ||
      !same(spec.argv, [
        "milestone",
        "update",
        "00000000-0000-4000-9000-000000003301",
        "--sort-order",
        " 0x10 ",
      ]) ||
      !same(fixture, C033_SORT_RADIX_FROZEN_GRAPHQL) ||
      golden?.deviationId !== "C033-FINITE-DECIMAL-INPUT" ||
      golden.candidate.argv != null ||
      golden.candidate.graphqlUserAgent !== RUST_USER_AGENT ||
      !same(expected?.exit, { code: 2 }) ||
      !same(expected?.stdout, { utf8: "" }) ||
      !same(expected?.fileEffects, []) ||
      expected?.stderr == null || !("utf8" in expected.stderr) ||
      !expected.stderr.utf8.startsWith("error: invalid value ") ||
      !expected.stderr.utf8.includes("expected a finite decimal number") ||
      !same(
        [...golden.approvedSurfaces].sort(),
        ["exit", "stdout", "stderr", "graphql-fixture", "graphql-user-agent"]
          .sort(),
      )
    ) {
      throw new SchemaError(
        `case ${spec.id}: mutation parser rejection differs from the exact C033 contract`,
      )
    }
    return {
      ...fixture,
      expectedRequests: 0,
      groups: [],
      expectedRecords: structuredClone(fixture.initialRecords),
    }
  }
  if (
    fixture.groups.length !== 1 || fixture.groups[0].mode !== "ordered" ||
    !same(fixture.initialRecords, fixture.expectedRecords)
  ) {
    throw new SchemaError(
      `case ${spec.id}: GraphQL delta needs one effect-free ordered group`,
    )
  }
  const original = fixture.groups[0].steps
  if (
    original.some((step) => {
      if (step.kind !== "graphql" || step.effects.length !== 0) return true
      return getOperationAST(
        parse(step.operation.document),
        step.operation.operationName,
      )?.operation !== "query"
    }) ||
    delta.steps.length > original.length
  ) {
    throw new SchemaError(
      `case ${spec.id}: GraphQL delta must be a prefix of effect-free query steps`,
    )
  }
  if (delta.steps.length === 0) {
    // The reviewed candidate makes no request at all. The server still runs
    // with no groups, so any request reaching it fails fixture comparison.
    if (
      original.length !== 1 &&
      !(original.length === 2 && isC038EmptyIdSource(spec))
    ) {
      throw new SchemaError(
        `case ${spec.id}: zero-request GraphQL delta needs exactly one frozen query step`,
      )
    }
    return { ...fixture, expectedRequests: 0, groups: [] }
  }
  let changedVariables = false
  const steps = delta.steps.map((entry, index) => {
    const source = original[index]
    if (source.kind !== "graphql" || entry.id !== source.id) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL delta step ${
          index + 1
        } is not the frozen prefix`,
      )
    }
    if (entry.variables == null) return { ...source }
    if (source.operation.variables == null) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL delta cannot add a variables key`,
      )
    }
    if (same(entry.variables, source.operation.variables)) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL delta variables are unchanged on ${source.id}`,
      )
    }
    changedVariables = true
    return {
      ...source,
      operation: { ...source.operation, variables: entry.variables },
    }
  })
  if (steps.length === original.length && !changedVariables) {
    throw new SchemaError(
      `case ${spec.id}: GraphQL delta does not change the fixture`,
    )
  }
  return {
    ...fixture,
    expectedRequests: steps.length,
    groups: [{ mode: "ordered", steps }],
  }
}

function changedSurfaces(spec: CaseSpec, golden: ReviewedGolden): string[] {
  const changed: string[] = []
  if (
    golden.candidate.argv != null && !same(spec.argv, golden.candidate.argv)
  ) {
    changed.push("argv")
  }
  const expected = golden.candidate.expected
  if (expected != null) {
    if (!same(spec.expected.exit, expected.exit)) changed.push("exit")
    if (!sameStdout(spec.expected.stdout, expected.stdout)) {
      changed.push("stdout")
    }
    if (
      !bytesEqual(
        decodeByteValue(spec.expected.stderr),
        decodeByteValue(expected.stderr),
      )
    ) {
      changed.push("stderr")
    }
    if (!same(spec.expected.fileEffects, expected.fileEffects)) {
      changed.push("files")
    }
  }
  if (golden.candidate.graphqlUserAgent != null) {
    changed.push("graphql-user-agent")
  }
  if (golden.candidate.graphql != null) changed.push("graphql-fixture")
  return changed
}

function changedSurfacesV2(spec: CaseSpec, golden: ReviewedGoldenV2): string[] {
  const changed = ["graphql-fixture", "graphql-user-agent"]
  const expected = golden.candidate.expected
  if (expected != null) {
    if (!same(spec.expected.exit, expected.exit)) changed.push("exit")
    if (!sameStdout(spec.expected.stdout, expected.stdout)) {
      changed.push("stdout")
    }
    if (
      !bytesEqual(
        decodeByteValue(spec.expected.stderr),
        decodeByteValue(expected.stderr),
      )
    ) {
      changed.push("stderr")
    }
    if (!same(spec.expected.fileEffects, expected.fileEffects)) {
      changed.push("files")
    }
  }
  return changed
}

function initiativePageInfo(data: unknown, label: string): {
  hasNextPage: boolean
  endCursor: string | null
} {
  if (
    !record(data) || Object.keys(data).length !== 1 || !record(data.initiatives)
  ) {
    throw new SchemaError(`${label}: response.data needs only initiatives`)
  }
  const connection = data.initiatives
  if (!Array.isArray(connection.nodes) || !record(connection.pageInfo)) {
    throw new SchemaError(`${label}: initiatives needs nodes and pageInfo`)
  }
  const pageInfo = connection.pageInfo
  if (
    typeof pageInfo.hasNextPage !== "boolean" ||
    !(pageInfo.endCursor === null || typeof pageInfo.endCursor === "string")
  ) {
    throw new SchemaError(`${label}: pageInfo has invalid shape`)
  }
  return { hasNextPage: pageInfo.hasNextPage, endCursor: pageInfo.endCursor }
}

function containsRecordReference(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsRecordReference)
  return record(value) && (
    Object.hasOwn(value, "$record") ||
    Object.values(value).some(containsRecordReference)
  )
}

function checkedPageOperation(
  frozenDocument: string,
  appendedDocument: string,
  operationName: string | undefined,
  label: string,
): void {
  const frozen = parse(frozenDocument)
  const appended = parse(appendedDocument)
  if (
    frozen.definitions.length !== 1 || appended.definitions.length !== 1 ||
    frozen.definitions[0].kind !== Kind.OPERATION_DEFINITION ||
    appended.definitions[0].kind !== Kind.OPERATION_DEFINITION
  ) throw new SchemaError(`${label}: needs one operation without fragments`)
  const base = frozen.definitions[0]
  const next = appended.definitions[0]
  if (
    base.operation !== "query" || next.operation !== "query" ||
    next.selectionSet.selections.length !== 1 ||
    base.selectionSet.selections.length !== 1 ||
    next.selectionSet.selections[0].kind !== Kind.FIELD ||
    base.selectionSet.selections[0].kind !== Kind.FIELD ||
    next.selectionSet.selections[0].name.value !== "initiatives" ||
    operationName != null && operationName !== next.name?.value
  ) throw new SchemaError(`${label}: appended query must select initiatives`)
  const field = next.selectionSet.selections[0]
  const baseField = base.selectionSet.selections[0]
  const afterArguments =
    field.arguments?.filter((entry) => entry.name.value === "after") ?? []
  const afterDefinitions =
    next.variableDefinitions?.filter((entry) =>
      entry.variable.name.value === "after"
    ) ?? []
  if (
    afterArguments.length !== 1 ||
    afterArguments[0].value.kind !== Kind.VARIABLE ||
    afterArguments[0].value.name.value !== "after" ||
    afterDefinitions.length !== 1 ||
    afterDefinitions[0].type.kind !== Kind.NAMED_TYPE ||
    afterDefinitions[0].type.name.value !== "String" ||
    afterDefinitions[0].defaultValue != null ||
    (afterDefinitions[0].directives?.length ?? 0) !== 0 ||
    (baseField.arguments ?? []).some((entry) => entry.name.value === "after") ||
    (base.variableDefinitions ?? []).some((entry) =>
      entry.variable.name.value === "after"
    )
  ) {
    throw new SchemaError(
      `${label}: needs exactly after:$after and $after:String`,
    )
  }
  const comparableBase = print({
    ...frozen,
    definitions: [{ ...base, name: undefined }],
  })
  const comparableNext = print({
    ...appended,
    definitions: [{
      ...next,
      name: undefined,
      variableDefinitions: next.variableDefinitions?.filter((entry) =>
        entry.variable.name.value !== "after"
      ),
      selectionSet: {
        ...next.selectionSet,
        selections: [{
          ...field,
          arguments: field.arguments?.filter((entry) =>
            entry.name.value !== "after"
          ),
        }],
      },
    }],
  })
  if (comparableNext !== comparableBase) {
    throw new SchemaError(
      `${label}: appended query changes the frozen selection or arguments`,
    )
  }
}

async function deriveInitiativePages(
  spec: CaseSpec,
  golden: ReviewedGoldenV2,
  path: string,
  pinned: PinnedGraphQLContext,
): Promise<NonNullable<CaseSpec["graphql"]>> {
  const fixture = spec.graphql
  const pages = golden.candidate.graphqlPages
  if (
    spec.route !== "linear initiative list" || fixture == null ||
    spec.fixtureServer != null || fixture.groups.length !== 1 ||
    fixture.groups[0].mode !== "ordered" ||
    fixture.groups[0].steps.length !== 1 ||
    !same(fixture.initialRecords, fixture.expectedRecords) ||
    Object.keys(fixture.initialRecords).length !== 0 ||
    fixture.expectedRequests !== 1
  ) {
    throw new SchemaError(
      `${path}: v2 needs one effect-free frozen initiative query`,
    )
  }
  const frozenStep = fixture.groups[0].steps[0]
  if (
    frozenStep.kind !== "graphql" || frozenStep.effects.length !== 0 ||
    frozenStep.response.kind !== "data" ||
    pages.retainedSteps[0] !== frozenStep.id
  ) throw new SchemaError(`${path}: v2 retained step differs from frozen query`)
  if (containsRecordReference(frozenStep.response.data)) {
    throw new SchemaError(`${path}: v2 cannot use record references`)
  }
  const firstPage = initiativePageInfo(frozenStep.response.data, path)
  if (!firstPage.hasNextPage || !firstPage.endCursor) {
    throw new SchemaError(
      `${path}: frozen first page needs a continuation cursor`,
    )
  }
  const repeat = spec.id === "c037-first-page-repeat-cursor-proposal"
  if (repeat) {
    const expected = golden.candidate.expected
    if (
      expected == null || !("code" in expected.exit) ||
      expected.exit.code === 0 || "mode" in expected.stdout ||
      decodeByteValue(expected.stdout).length !== 0
    ) {
      throw new SchemaError(
        `${path}: repeated-cursor candidate must exit nonzero with empty stdout`,
      )
    }
  }
  if (
    pages.appendedSteps.length !== (repeat ? 2 : 1) ||
    pages.expectedRequests !== 1 + pages.appendedSteps.length
  ) throw new SchemaError(`${path}: appended page count differs from case`)
  const seen = new Set([frozenStep.id])
  const baseVariables = frozenStep.operation.variables ?? {}
  let cursor = firstPage.endCursor
  const schema = await pinned.schema()
  const appended = []
  for (const [index, page] of pages.appendedSteps.entries()) {
    if (seen.has(page.id)) {
      throw new SchemaError(`${path}: duplicate appended step id`)
    }
    seen.add(page.id)
    const label = `${path}: ${page.id}`
    const serialized = JSON.stringify(page)
    if (
      serialized.includes("{{") || /lin_(api|oauth)_(?!fake)/i.test(serialized)
    ) {
      throw new SchemaError(
        `${label}: appended page contains forbidden literal`,
      )
    }
    try {
      checkedPageOperation(
        frozenStep.operation.document,
        page.operation.document,
        page.operation.operationName,
        label,
      )
    } catch (error) {
      throw new SchemaError(
        `${label}: invalid appended query: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    }
    const vars = page.operation.variables
    if (
      typeof vars.after !== "string" || vars.after.length === 0 ||
      vars.after !== cursor ||
      Object.keys(vars).length !== Object.keys(baseVariables).length + 1 ||
      !Object.entries(baseVariables).every(([key, value]) =>
        Object.hasOwn(vars, key) && same(vars[key], value)
      )
    ) {
      throw new SchemaError(
        `${label}: variables differ from frozen page or preceding cursor`,
      )
    }
    if (containsRecordReference(page.response.data)) {
      throw new SchemaError(`${label}: record reference is forbidden`)
    }
    const info = initiativePageInfo(page.response.data, label)
    if (repeat) {
      if (
        !info.hasNextPage || !info.endCursor ||
        index === 0 && info.endCursor === firstPage.endCursor ||
        index === 1 &&
          (info.endCursor !== firstPage.endCursor ||
            cursor === firstPage.endCursor)
      ) throw new SchemaError(`${label}: repeat case needs A→B→A cursors`)
    } else if (info.hasNextPage) {
      throw new SchemaError(`${label}: final appended page must be terminal`)
    }
    cursor = info.endCursor ?? ""
    const step: GraphQLStepSpec = {
      kind: "graphql",
      id: page.id,
      operation: page.operation,
      identity: structuredClone(frozenStep.identity),
      response: page.response,
      effects: [],
    }
    try {
      await projectGraphQLResponse(
        schema,
        new GraphQLState({}),
        step,
        page.operation.document,
        vars,
        page.operation.operationName,
      )
    } catch (error) {
      throw new SchemaError(
        `${label}: invalid projected response: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    }
    appended.push(step)
  }
  const candidateGraphql: NonNullable<CaseSpec["graphql"]> = {
    ...fixture,
    expectedRequests: pages.expectedRequests,
    groups: [{ mode: "ordered", steps: [frozenStep, ...appended] }],
  }
  const candidateSpec: CaseSpec = {
    ...spec,
    expected: golden.candidate.expected ?? spec.expected,
    graphql: candidateGraphql,
  }
  parseCase(candidateSpec)
  await checkGraphQLFixture(candidateSpec, path, pinned)
  resolveCase(candidateSpec, {
    home: "h",
    configHome: "c",
    cwd: "w",
    cwdRoot: "r",
    bin: "b",
    denoDir: "d",
    fixturePort: "0",
    referenceModuleUrl: "file:///reference",
  }, golden.candidate.graphqlUserAgent)
  return candidateGraphql
}

/** The reviewed Rust diagnostic may replace a source-only module URL stack. */
function candidateSubstitutions(
  spec: CaseSpec,
  golden: ReviewedGolden,
): CaseSpec["substitutions"] {
  const expected = golden.candidate.expected
  if (expected == null) return spec.substitutions
  const hasReferenceToken = [expected.stdout, expected.stderr].some((field) =>
    "utf8" in field && field.utf8.includes("{{referenceModuleUrl}}")
  )
  return hasReferenceToken
    ? spec.substitutions
    : spec.substitutions.filter((name) => name !== "referenceModuleUrl")
}

async function loadReviewedBinding(
  dir: string,
  spec: CaseSpec,
  pinned: PinnedGraphQLContext,
): Promise<
  | { v1: NonNullable<LoadedCase["golden"]>; v2?: never }
  | { v2: NonNullable<LoadedCase["goldenV2"]>; v1?: never }
  | null
> {
  const binding = spec.deviation
  if (binding == null) return null
  const root = join(await Deno.realPath(dir), "rust-goldens")
  const contractDir = join(root, binding.contract)
  const path = join(contractDir, `${spec.id}.json`)
  const relativePath = relative(root, path)
  if (relativePath.startsWith("..") || relativePath.startsWith("/")) {
    throw new SchemaError(`case ${spec.id}: reviewed golden escapes case root`)
  }
  for (const directory of [root, contractDir]) {
    const info = await Deno.lstat(directory).catch(() => null)
    if (info == null || info.isSymlink || !info.isDirectory) {
      throw new SchemaError(
        `case ${spec.id}: reviewed golden directory is missing or unsafe: ${directory}`,
      )
    }
  }
  const info = await Deno.lstat(path).catch(() => null)
  if (info == null || info.isSymlink || !info.isFile) {
    throw new SchemaError(
      `case ${spec.id}: reviewed golden is missing or unsafe: ${path}`,
    )
  }
  const bytes = await Deno.readFile(path)
  const hash = await sha256Hex(bytes)
  if (hash !== binding.sha256) {
    throw new SchemaError(
      `case ${spec.id}: reviewed golden SHA-256 differs from case pin`,
    )
  }
  let parsed: unknown
  try {
    parsed = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes))
  } catch {
    throw new SchemaError(`case ${spec.id}: reviewed golden is not UTF-8 JSON`)
  }
  if (!record(parsed) || !Number.isInteger(parsed.formatVersion)) {
    throw new SchemaError(
      `${path}: reviewed golden formatVersion must be 1 or 2`,
    )
  }
  if (parsed.formatVersion === 2) {
    const v2 = parseReviewedGoldenV2(parsed, path)
    if (
      v2.caseId !== spec.id || v2.deviationId !== binding.id ||
      v2.contract !== binding.contract
    ) throw new SchemaError(`${path}: v2 identity differs from case binding`)
    if (JSON.stringify(v2.candidate).includes("{{referenceModuleUrl}}")) {
      throw new SchemaError(`${path}: v2 cannot use referenceModuleUrl`)
    }
    const surfaces = changedSurfacesV2(spec, v2)
    if (
      v2.candidate.expected != null && surfaces.length === 2 ||
      !same([...surfaces].sort(), [...v2.approvedSurfaces].sort())
    ) throw new SchemaError(`${path}: v2 approvedSurfaces differ from changes`)
    const checkedFixture = await deriveInitiativePages(spec, v2, path, pinned)
    return { v2: { spec: v2, sha256: hash, checkedFixture } }
  }
  if (parsed.formatVersion !== 1) {
    throw new SchemaError(
      `${path}: reviewed golden formatVersion must be 1 or 2`,
    )
  }
  const golden = parseReviewedGolden(parsed, path)
  if (JSON.stringify(golden.candidate).includes("{{referenceModuleUrl}}")) {
    throw new SchemaError(
      `case ${spec.id}: Rust golden cannot use referenceModuleUrl`,
    )
  }
  if (
    golden.candidate.argv != null &&
    JSON.stringify(golden.candidate.argv).includes("{{cwdRoot}}")
  ) {
    throw new SchemaError(
      `case ${spec.id}: Rust golden cwdRoot is restricted to expected output`,
    )
  }
  if (
    golden.candidate.expected?.fileEffects != null &&
    JSON.stringify(golden.candidate.expected.fileEffects).includes(
      "{{cwdRoot}}",
    )
  ) {
    throw new SchemaError(
      `case ${spec.id}: Rust golden cwdRoot is restricted to expected output`,
    )
  }
  if (
    golden.caseId !== spec.id || golden.deviationId !== binding.id ||
    golden.contract !== binding.contract
  ) {
    throw new SchemaError(
      `case ${spec.id}: reviewed golden identity differs from case binding`,
    )
  }
  const delta = golden.candidate.graphql
  if (delta != null) {
    const serialized = JSON.stringify(delta)
    if (
      serialized.includes("{{") || /lin_(api|oauth)_(?!fake)/i.test(serialized)
    ) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL delta cannot contain placeholders or credentials`,
      )
    }
    if (golden.candidate.graphqlUserAgent == null) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL delta requires the Rust User-Agent binding`,
      )
    }
  }
  const candidateGraphql = applyGraphQLDelta(spec, delta, golden)
  const actual = changedSurfaces(spec, golden)
  if (
    golden.candidate.argv != null && !actual.includes("argv") ||
    golden.candidate.expected != null &&
      !actual.some((surface) =>
        ["exit", "stdout", "stderr", "files"].includes(surface)
      )
  ) {
    throw new SchemaError(
      `case ${spec.id}: redundant unchanged candidate override`,
    )
  }
  if (
    !same([...actual].sort(), [...golden.approvedSurfaces].sort())
  ) {
    throw new SchemaError(
      `case ${spec.id}: approvedSurfaces do not match actual changed surfaces`,
    )
  }
  if (spec.graphql == null && golden.candidate.graphqlUserAgent != null) {
    throw new SchemaError(
      `case ${spec.id}: GraphQL User-Agent override requires a GraphQL fixture`,
    )
  }
  if (golden.candidate.graphqlUserAgent != null) {
    const steps = spec.graphql?.groups.flatMap((group) =>
      group.mode === "ordered"
        ? group.steps
        : group.lanes.flatMap((lane) =>
          lane.steps
        )
    ) ?? []
    if (!steps.some((step) => step.kind === "graphql")) {
      throw new SchemaError(
        `case ${spec.id}: GraphQL User-Agent override has no GraphQL request`,
      )
    }
  }
  const candidateSpec: CaseSpec = {
    ...spec,
    argv: golden.candidate.argv ?? spec.argv,
    expected: golden.candidate.expected ?? spec.expected,
    substitutions: candidateSubstitutions(spec, golden),
    graphql: candidateGraphql,
  }
  if (delta != null) {
    if (delta.steps.length === 0) {
      // Ordinary candidate fields keep the frozen case checks against the
      // original fixture; only the derived fixture uses the zero schema.
      parseCase({ ...candidateSpec, graphql: spec.graphql })
      const zero = v.safeParse(
        ZeroRequestCandidateGraphQLSchema,
        candidateGraphql,
      )
      if (!zero.success) {
        throw new SchemaError(
          `case ${spec.id}: zero-request candidate GraphQL fixture is invalid`,
        )
      }
    } else {
      parseCase(candidateSpec)
    }
    await checkGraphQLFixture(candidateSpec, path, pinned)
  }
  const stdout = candidateSpec.expected.stdout
  if (
    "mode" in stdout && stdout.mode === "close-after-bytes" &&
    stdout.count > spec.outputCapBytes
  ) {
    throw new SchemaError(
      `case ${spec.id}: candidate close-after-bytes exceeds outputCapBytes`,
    )
  }
  // Resolve candidate placeholders before any selected baseline can execute.
  resolveCase(candidateSpec, {
    home: "h",
    configHome: "c",
    cwd: "w",
    cwdRoot: "r",
    bin: "b",
    denoDir: "d",
    fixturePort: "0",
    referenceModuleUrl: "file:///reference",
  }, golden.candidate.graphqlUserAgent)
  return { v1: { spec: golden, sha256: hash } }
}

async function checkGoldenTree(
  dir: string,
  boundIds: ReadonlySet<string>,
): Promise<void> {
  const root = join(await Deno.realPath(dir), "rust-goldens")
  const rootInfo = await Deno.lstat(root).catch(() => null)
  if (rootInfo == null) {
    if (boundIds.size !== 0) {
      throw new SchemaError("reviewed golden root is missing")
    }
    return
  }
  if (rootInfo.isSymlink || !rootInfo.isDirectory) {
    throw new SchemaError("reviewed golden root is unsafe")
  }
  for await (const contract of Deno.readDir(root)) {
    if (
      contract.name !== RUST_CONTRACT || contract.isSymlink ||
      !contract.isDirectory
    ) {
      throw new SchemaError(
        `unknown or unsafe reviewed golden contract ${contract.name}`,
      )
    }
    for await (const entry of Deno.readDir(join(root, contract.name))) {
      if (entry.isSymlink || !entry.isFile || !entry.name.endsWith(".json")) {
        throw new SchemaError(`unsafe reviewed golden entry ${entry.name}`)
      }
      const id = entry.name.slice(0, -5)
      if (!boundIds.has(id)) {
        throw new SchemaError(`orphan reviewed golden ${entry.name}`)
      }
    }
  }
}

export function candidateCaseView(loaded: LoadedCase): LoadedCase {
  if (loaded.golden != null && loaded.goldenV2 != null) {
    throw new SchemaError(
      `case ${loaded.spec.id}: both golden versions are bound`,
    )
  }
  if (loaded.goldenV2 != null) {
    if (!checkedV2Cases.has(loaded) || loaded.goldenV2.checkedFixture == null) {
      throw new SchemaError(`case ${loaded.spec.id}: unvalidated v2 candidate`)
    }
    return {
      ...loaded,
      spec: {
        ...loaded.spec,
        expected: loaded.goldenV2.spec.candidate.expected ??
          loaded.spec.expected,
        graphql: loaded.goldenV2.checkedFixture,
      },
      runtimeUserAgent: loaded.goldenV2.spec.candidate.graphqlUserAgent,
    }
  }
  const golden = loaded.golden?.spec
  if (golden == null) {
    if (loaded.spec.deviation != null || loaded.spec.graphql != null) {
      throw new SchemaError(
        `case ${loaded.spec.id}: Rust candidate requires a loaded reviewed golden`,
      )
    }
    return { ...loaded, spec: { ...loaded.spec } }
  }
  return {
    ...loaded,
    spec: {
      ...loaded.spec,
      argv: golden.candidate.argv ?? loaded.spec.argv,
      expected: golden.candidate.expected ?? loaded.spec.expected,
      substitutions: candidateSubstitutions(loaded.spec, golden),
      graphql: applyGraphQLDelta(loaded.spec, golden.candidate.graphql, golden),
    },
    runtimeUserAgent: golden.candidate.graphqlUserAgent,
  }
}

function rewriteUserAgent(
  fixture: RuntimeGraphQLFixtureSpec,
  userAgent: typeof RUST_USER_AGENT,
): RuntimeGraphQLFixtureSpec {
  return {
    ...fixture,
    groups: fixture.groups.map((group) =>
      group.mode === "ordered"
        ? {
          ...group,
          steps: group.steps.map((step) =>
            step.kind === "graphql"
              ? { ...step, identity: { ...step.identity, userAgent } }
              : step
          ),
        }
        : {
          ...group,
          lanes: group.lanes.map((lane) => ({
            ...lane,
            steps: lane.steps.map((step) =>
              step.kind === "graphql"
                ? { ...step, identity: { ...step.identity, userAgent } }
                : step
            ),
          })),
        }
    ),
  }
}

export const LANE_CLEANUP_MARGIN_MS = 1000

export function checkLaneDeadline(
  spec: CaseSpec,
  effectiveTimeoutMs: number,
): void {
  const total = spec.graphql?.groups.reduce(
    (sum, group) => sum + (group.mode === "lanes" ? group.timeoutMs : 0),
    0,
  ) ?? 0
  if (total > 0 && total + LANE_CLEANUP_MARGIN_MS >= effectiveTimeoutMs) {
    throw new SchemaError(
      `case ${spec.id}: lane deadlines plus cleanup margin must be strictly below timeoutMs`,
    )
  }
}

function headersConflict(
  a: Readonly<Record<string, string>>,
  b: Readonly<Record<string, string>>,
): boolean {
  for (const [name, value] of Object.entries(a)) {
    const other = Object.entries(b).find(([key]) =>
      key.toLowerCase() === name.toLowerCase()
    )
    if (other != null && other[1] !== value) return true
  }
  return false
}

function firstStepsOverlap(
  a: InteractionSpec,
  b: InteractionSpec,
  schema: GraphQLSchema,
): boolean {
  if (a.kind !== b.kind) return false
  if (a.kind === "asset" && b.kind === "asset") {
    return a.method === b.method && a.path === b.path &&
      !headersConflict(a.requiredHeaders, b.requiredHeaders) &&
      !a.forbiddenHeaders.some((name) =>
        Object.keys(b.requiredHeaders).some((key) =>
          key.toLowerCase() === name.toLowerCase()
        )
      ) &&
      !b.forbiddenHeaders.some((name) =>
        Object.keys(a.requiredHeaders).some((key) =>
          key.toLowerCase() === name.toLowerCase()
        )
      )
  }
  if (a.kind !== "graphql" || b.kind !== "graphql") {
    throw new Error("unexpected interaction kind")
  }
  if (
    a.identity.authorization !== b.identity.authorization ||
    headersConflict(a.identity.headers, b.identity.headers)
  ) return false
  if (
    a.response.kind === "validationErrors" ||
    b.response.kind === "validationErrors"
  ) {
    return a.operation.document.trim() === b.operation.document.trim()
  }
  return matchGraphQL(a.operation, b.operation, schema).matches ||
    matchGraphQL(b.operation, a.operation, schema).matches
}

function sameEffectFreeLane(
  a: readonly InteractionSpec[],
  b: readonly InteractionSpec[],
): boolean {
  const shape = (steps: readonly InteractionSpec[]) =>
    steps.map(({ id: _id, ...step }) => step)
  return JSON.stringify(shape(a)) === JSON.stringify(shape(b)) &&
    a.every((step) => step.kind !== "graphql" || step.effects.length === 0)
}

interface TypedRecordRead {
  key: string
  type: GraphQLType
}

function laneDependencies(
  steps: readonly InteractionSpec[],
  schema: GraphQLSchema,
  recordValues: ReadonlyMap<string, readonly unknown[]>,
): { writes: Set<string>; reads: Set<string> } {
  const writes = new Set<string>()
  const reads = new Set<string>()
  const typedReads: TypedRecordRead[] = []
  for (const step of steps) {
    if (step.kind !== "graphql") continue
    for (const effect of step.effects) {
      writes.add(effect.record)
    }
    if (
      step.response.kind === "data" || step.response.kind === "graphqlErrors"
    ) {
      const operation = getOperationAST(
        parse(step.operation.document),
        step.operation.operationName,
      )
      const root = operation?.operation === "query"
        ? schema.getQueryType()
        : operation?.operation === "mutation"
        ? schema.getMutationType()
        : schema.getSubscriptionType()
      if (root != null) {
        checkResponseReferences(
          step.response.data,
          root,
          schema,
          null,
          step.id,
          reads,
          typedReads,
        )
      }
    }
  }
  // A returned record can itself point at another record. Every stored version
  // is possible while lanes interleave, so close reads over initial and after
  // values before comparing them with writes in another lane.
  const visited = new Set<string>()
  for (let index = 0; index < typedReads.length; index++) {
    const { key, type } = typedReads[index]
    const visit = `${key}\u0000${getNamedType(type).name}`
    if (visited.has(visit)) continue
    visited.add(visit)
    for (const value of recordValues.get(key) ?? []) {
      checkResponseReferences(
        value,
        type,
        schema,
        null,
        `record ${key}`,
        reads,
        typedReads,
      )
    }
  }
  return { writes, reads }
}

function checkRedirects(steps: readonly InteractionSpec[], file: string): void {
  for (const [index, step] of steps.entries()) {
    if (step.kind !== "asset" || step.response.location == null) continue
    const next = steps[index + 1]
    if (
      next?.kind !== "asset" || next.path !== step.response.location ||
      next.fixedHost !== step.fixedHost
    ) {
      throw new SchemaError(
        `${file}: asset redirect must target the immediately following same-host step in the same lane or group`,
      )
    }
  }
}

function fixedHostUrls(value: unknown, found: Set<string>): void {
  if (typeof value === "string") {
    for (
      const match of value.matchAll(
        /https:\/\/(?:uploads|public)\.linear\.app\/[^\s)"']+/g,
      )
    ) {
      const url = new URL(match[0])
      found.add(`${url.hostname}${url.pathname}${url.search}`)
    }
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) fixedHostUrls(item, found)
    return
  }
  if (record(value)) {
    for (const item of Object.values(value)) fixedHostUrls(item, found)
  }
}

async function inspectConfigFixture(
  directory: string,
  file: string,
): Promise<void> {
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const info = await Deno.lstat(child)
      if (info.isSymlink || (!info.isDirectory && !info.isFile)) {
        throw new SchemaError(
          `${file}: configFixture contains a symlink or unsupported entry`,
        )
      }
      if (info.isDirectory) await walk(child)
      else {
        let contents: string
        try {
          contents = new TextDecoder("utf-8", { fatal: true }).decode(
            await Deno.readFile(child),
          )
        } catch {
          throw new SchemaError(
            `${file}: configFixture must contain UTF-8 text files`,
          )
        }
        if (/lin_(?:api|oauth)_(?!fake)/i.test(contents)) {
          throw new SchemaError(
            `${file}: configFixture contains a non-fake Linear credential`,
          )
        }
      }
    }
  }
  await walk(directory)
}

export interface ResolvedCase {
  argv: string[]
  stdin: Uint8Array
  env: Record<string, string>
  expected: {
    exit: CaseSpec["expected"]["exit"]
    stdout: Uint8Array
    stdoutMode: StdoutMode
    stderr: Uint8Array
    fileEffects: CaseSpec["expected"]["fileEffects"]
  }
  fixtureServer: CaseSpec["fixtureServer"]
  graphql: RuntimeGraphQLFixtureSpec | null | undefined
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function checkResponseReferences(
  value: unknown,
  type: GraphQLType,
  schema: GraphQLSchema,
  known: ReadonlySet<string> | null,
  label: string,
  found: Set<string> | null = null,
  typedReads: TypedRecordRead[] | null = null,
): void {
  if (isNonNullType(type)) {
    return checkResponseReferences(
      value,
      type.ofType,
      schema,
      known,
      label,
      found,
      typedReads,
    )
  }
  if (value == null) return
  if (isListType(type)) {
    if (Array.isArray(value)) {
      value.forEach((item, index) =>
        checkResponseReferences(
          item,
          type.ofType,
          schema,
          known,
          `${label}[${index}]`,
          found,
          typedReads,
        )
      )
    }
    return
  }
  const named = getNamedType(type)
  if (!isObjectType(named) && !isInterfaceType(named) && !isUnionType(named)) {
    return
  }
  if (!record(value)) return
  if (Object.hasOwn(value, "$record")) {
    if (Object.keys(value).length !== 1 || typeof value.$record !== "string") {
      throw new SchemaError(`${label}: malformed composite $record reference`)
    }
    if (known != null && !known.has(value.$record)) {
      throw new SchemaError(`${label}: unknown composite $record reference`)
    }
    found?.add(value.$record)
    typedReads?.push({ key: value.$record, type })
    return
  }
  const concrete = (isUnionType(named) || isInterfaceType(named)) &&
      typeof value.__typename === "string"
    ? schema.getType(value.__typename)
    : named
  if (!isObjectType(concrete) && !isInterfaceType(concrete)) return
  for (const [key, child] of Object.entries(value)) {
    const field = concrete.getFields()[key]
    if (field != null) {
      checkResponseReferences(
        child,
        field.type,
        schema,
        known,
        `${label}.${key}`,
        found,
        typedReads,
      )
    }
  }
}

interface PinnedGraphQLContext {
  digests(): Promise<{ hash: string; baselineHash: string }>
  schema(): Promise<GraphQLSchema>
}

function pinnedGraphQLContext(): PinnedGraphQLContext {
  let loaded:
    | Promise<{ sdl: string; hash: string; baselineHash: string }>
    | undefined
  let built: Promise<GraphQLSchema> | undefined
  const digests = () =>
    loaded ??= (async () => {
      const root = join(import.meta.dirname ?? ".", "../../..")
      const sdl = await Deno.readTextFile(join(root, "graphql/schema.graphql"))
      const hash = await sha256Hex(new TextEncoder().encode(sdl))
      const baseline = v.parse(
        v.object({ schemaSha256: v.string() }),
        JSON.parse(
          await Deno.readTextFile(join(root, "rust/parity/baseline.json")),
        ),
      )
      return { sdl, hash, baselineHash: baseline.schemaSha256 }
    })()
  return {
    digests,
    schema: () => built ??= digests().then(({ sdl }) => buildPinnedSchema(sdl)),
  }
}

async function checkGraphQLFixture(
  spec: CaseSpec,
  file: string,
  pinned: PinnedGraphQLContext,
): Promise<void> {
  const fixture = spec.graphql
  if (fixture == null) return
  const { hash, baselineHash } = await pinned.digests()
  if (hash !== baselineHash || fixture.schemaSha256 !== hash) {
    throw new SchemaError(
      `${file}: GraphQL schema digest differs from pinned baseline`,
    )
  }
  const schema = await pinned.schema()
  checkLaneDeadline(spec, spec.timeoutMs)
  const recordValues = new Map<string, unknown[]>()
  for (const [key, value] of Object.entries(fixture.initialRecords)) {
    recordValues.set(key, [value])
  }
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (step.kind !== "graphql") continue
      for (const effect of step.effects) {
        if (effect.kind !== "put") continue
        const values = recordValues.get(effect.record) ?? []
        values.push(effect.after)
        recordValues.set(effect.record, values)
      }
    }
  }
  for (const group of fixture.groups) {
    if (group.mode === "ordered") {
      checkRedirects(group.steps, file)
      continue
    }
    for (const lane of group.lanes) checkRedirects(lane.steps, file)
    for (let left = 0; left < group.lanes.length; left++) {
      for (let right = left + 1; right < group.lanes.length; right++) {
        const a = group.lanes[left]
        const b = group.lanes[right]
        if (
          firstStepsOverlap(a.steps[0], b.steps[0], schema) &&
          !sameEffectFreeLane(a.steps, b.steps)
        ) {
          throw new SchemaError(
            `${file}: ambiguous non-identical lane first steps`,
          )
        }
        const aa = laneDependencies(a.steps, schema, recordValues)
        const bb = laneDependencies(b.steps, schema, recordValues)
        if (
          [...aa.writes].some((key) =>
            bb.writes.has(key) || bb.reads.has(key)
          ) ||
          [...bb.writes].some((key) => aa.reads.has(key))
        ) {
          throw new SchemaError(
            `${file}: cross-lane record read/write dependency is unsupported`,
          )
        }
      }
    }
  }
  const knownRecords = new Set(Object.keys(fixture.initialRecords))
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (step.kind !== "graphql") continue
      for (const effect of step.effects) {
        if (effect.kind === "put") knownRecords.add(effect.record)
      }
    }
  }
  const seen = new Set<string>()
  const assets = new Set<string>()
  const fixedAssets = new Set<string>()
  for (const group of fixture.groups) {
    const lanes = group.mode === "ordered"
      ? [{ id: "ordered", steps: group.steps }]
      : group.lanes
    if (new Set(lanes.map((lane) => lane.id)).size !== lanes.length) {
      throw new SchemaError(`${file}: duplicate lane id`)
    }
    const concurrentRecords = new Map<string, string>()
    for (const lane of lanes) {
      for (const step of lane.steps) {
        if (seen.has(step.id)) {
          throw new SchemaError(`${file}: duplicate interaction id ${step.id}`)
        }
        seen.add(step.id)
        if (step.kind === "asset") {
          assets.add(step.path)
          if (step.fixedHost != null) {
            fixedAssets.add(`${step.fixedHost}${step.path}`)
          }
          if (
            step.method === "GET" && decodeByteValue(step.body).length !== 0
          ) throw new SchemaError(`${file}: GET asset body must be empty`)
          continue
        }
        if (step.response.kind !== "validationErrors") {
          try {
            const selfMatch = matchGraphQL(
              step.operation,
              step.operation,
              schema,
            )
            if (!selfMatch.matches) {
              throw new Error(
                selfMatch.reason ?? "fixture operation does not match itself",
              )
            }
          } catch (error) {
            throw new SchemaError(
              `${file}: ${step.id}: ${
                error instanceof Error ? error.message : String(error)
              }`,
            )
          }
        }
        if (
          step.response.kind === "data" ||
          step.response.kind === "graphqlErrors"
        ) {
          const operation = getOperationAST(
            parse(step.operation.document),
            step.operation.operationName,
          )
          const root = operation?.operation === "query"
            ? schema.getQueryType()
            : operation?.operation === "mutation"
            ? schema.getMutationType()
            : schema.getSubscriptionType()
          if (root == null) {
            throw new SchemaError(
              `${file}: ${step.id}: GraphQL operation has no root type`,
            )
          }
          checkResponseReferences(
            step.response.data,
            root,
            schema,
            knownRecords,
            `${file}: ${step.id}: response.data`,
          )
        }
        for (const effect of step.effects) {
          if (
            group.mode === "lanes" && concurrentRecords.has(effect.record) &&
            concurrentRecords.get(effect.record) !== lane.id
          ) {
            throw new SchemaError(
              `${file}: concurrent effects on ${effect.record} are unsupported`,
            )
          }
          concurrentRecords.set(effect.record, lane.id)
        }
      }
    }
  }
  if (fixedAssets.size > 0) {
    const urls = new Set<string>()
    for (const group of fixture.groups) {
      const steps = group.mode === "ordered"
        ? group.steps
        : group.lanes.flatMap((lane) => lane.steps)
      for (const step of steps) {
        if (
          step.kind === "graphql" &&
          (step.response.kind === "data" ||
            step.response.kind === "graphqlErrors")
        ) fixedHostUrls(step.response.data, urls)
      }
    }
    for (const url of urls) {
      if (!fixedAssets.has(url)) {
        throw new SchemaError(
          `${file}: fixed-host GraphQL URL has no declared asset interaction`,
        )
      }
    }
  }
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (
        step.kind === "asset" && step.response.location != null &&
        !assets.has(step.response.location)
      ) {
        throw new SchemaError(
          `${file}: asset redirect target ${step.response.location} is undeclared`,
        )
      }
    }
  }
}

export async function loadCases(
  dir: string,
  manifestRoutes: ReadonlySet<string>,
  filter?: string,
  contract: CandidateContract = FROZEN_CONTRACT,
): Promise<LoadedCase[]> {
  const pinned = pinnedGraphQLContext()
  const files: string[] = []
  for await (const entry of Deno.readDir(dir)) {
    if (entry.isFile && entry.name.endsWith(".json")) files.push(entry.name)
  }
  files.sort()
  const seen = new Set<string>()
  const boundIds = new Set<string>()
  const loaded: LoadedCase[] = []
  for (const name of files) {
    const file = join(dir, name)
    let parsed: unknown
    try {
      parsed = JSON.parse(await Deno.readTextFile(file))
    } catch (error) {
      throw new SchemaError(
        `${file}: invalid JSON: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    }
    const spec = parseCase(parsed, file)
    await checkGraphQLFixture(spec, file, pinned)
    if (spec.graphql != null) {
      const steps = spec.graphql.groups.flatMap((group) =>
        group.mode === "ordered"
          ? group.steps
          : group.lanes.flatMap((lane) => lane.steps)
      )
      if (
        steps.some((step) =>
          step.kind === "graphql" &&
          step.identity.userAgent !== FROZEN_USER_AGENT
        )
      ) {
        throw new SchemaError(
          `${file}: baseline GraphQL User-Agent differs from frozen 2.6.0`,
        )
      }
    }
    if (`${spec.id}.json` !== name) {
      throw new SchemaError(
        `${file}: id ${spec.id} does not match the file name`,
      )
    }
    if (seen.has(spec.id)) {
      throw new SchemaError(`${file}: duplicate case id ${spec.id}`)
    }
    seen.add(spec.id)
    if (!manifestRoutes.has(spec.route)) {
      throw new SchemaError(
        `${file}: route "${spec.route}" is not in rust/parity/manifest.json`,
      )
    }
    const binding = await loadReviewedBinding(dir, spec, pinned)
    const golden = binding?.v1 ?? null
    const goldenV2 = binding?.v2 ?? null
    if (golden != null && goldenV2 != null) {
      throw new SchemaError(`${file}: both reviewed golden versions are bound`)
    }
    if (binding != null) boundIds.add(spec.id)
    if (
      contract === RUST_CONTRACT && spec.graphql != null &&
      golden?.spec.candidate.graphqlUserAgent !== RUST_USER_AGENT &&
      goldenV2?.spec.candidate.graphqlUserAgent !== RUST_USER_AGENT
    ) {
      throw new SchemaError(
        `${file}: Rust contract requires exact GraphQL User-Agent binding`,
      )
    }
    let fixtureDir: string | null = null
    if (spec.cwdFixture !== "empty") {
      fixtureDir = join(dir, "fixtures", spec.cwdFixture)
      const info = await Deno.lstat(fixtureDir).catch(() => null)
      if (info == null || !info.isDirectory) {
        throw new SchemaError(
          `${file}: cwd fixture directory ${fixtureDir} is missing`,
        )
      }
    }
    let configFixtureDir: string | null = null
    if (spec.configFixture != null) {
      configFixtureDir = join(dir, "fixtures", spec.configFixture)
      const info = await Deno.lstat(configFixtureDir).catch(() => null)
      if (info == null || !info.isDirectory || info.isSymlink) {
        throw new SchemaError(
          `${file}: config fixture directory is missing or unsafe`,
        )
      }
      await inspectConfigFixture(configFixtureDir, file)
    }
    // Resolve with dummy values now so undeclared placeholders fail at load time.
    resolveCase(spec, {
      home: "h",
      configHome: "c",
      cwd: "w",
      cwdRoot: "r",
      bin: "b",
      denoDir: "d",
      fixturePort: "0",
      referenceModuleUrl: "file:///reference",
    })
    if (filter == null || spec.id.includes(filter)) {
      const item: LoadedCase = {
        file,
        spec,
        fixtureDir,
        configFixtureDir,
        golden,
        goldenV2,
      }
      if (goldenV2 != null) checkedV2Cases.add(item)
      loaded.push(item)
    }
  }
  await checkGoldenTree(dir, boundIds)
  return loaded
}

function resolveBytes(
  value: CaseSpec["stdin"],
  declared: readonly SubstitutionName[],
  values: Readonly<Record<SubstitutionName, string>>,
  label: string,
): Uint8Array {
  if ("utf8" in value) {
    return decodeByteValue({
      utf8: substitute(value.utf8, declared, values, label),
    })
  }
  return decodeByteValue(value)
}

function substituteValues(
  value: unknown,
  declared: readonly SubstitutionName[],
  values: Readonly<Record<SubstitutionName, string>>,
  label: string,
): unknown {
  if (typeof value === "string") {
    return substitute(value, declared, values, label)
  }
  if (Array.isArray(value)) {
    return value.map((entry, index) =>
      substituteValues(entry, declared, values, `${label}[${index}]`)
    )
  }
  if (typeof value === "object" && value != null) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        substituteValues(entry, declared, values, `${label}.${key}`),
      ]),
    )
  }
  return value
}

export function resolveCase(
  spec: CaseSpec,
  values: Readonly<Record<SubstitutionName, string>>,
  userAgent?: typeof RUST_USER_AGENT,
): ResolvedCase {
  const declared = spec.substitutions
  const label = `case ${spec.id}`
  const env: Record<string, string> = {}
  for (const [key, value] of Object.entries(spec.env)) {
    env[key] = substitute(value, declared, values, `${label} env ${key}`)
  }
  const stdout = spec.expected.stdout
  const stdoutMode: StdoutMode = "mode" in stdout
    ? stdout.mode === "closed-at-start"
      ? { mode: "closed-at-start" }
      : { mode: "close-after-bytes", count: stdout.count }
    : { mode: "drain" }
  const expectedStdout = "mode" in stdout
    ? stdout.mode === "closed-at-start"
      ? new Uint8Array()
      : decodeByteValue(stdout.prefix)
    : resolveBytes(stdout, declared, values, `${label} expected stdout`)
  return {
    argv: spec.argv.map((arg, index) =>
      substitute(arg, declared, values, `${label} argv[${index}]`)
    ),
    stdin: resolveBytes(spec.stdin, declared, values, `${label} stdin`),
    env,
    expected: {
      exit: spec.expected.exit,
      stdout: expectedStdout,
      stdoutMode,
      stderr: resolveBytes(
        spec.expected.stderr,
        declared,
        values,
        `${label} expected stderr`,
      ),
      fileEffects: spec.expected.fileEffects,
    },
    fixtureServer: spec.fixtureServer == null ? null : {
      ...spec.fixtureServer,
      responses: spec.fixtureServer.responses.map((response, index) => ({
        ...response,
        body: "utf8" in response.body
          ? {
            utf8: substitute(
              response.body.utf8,
              declared,
              values,
              `${label} fixture response ${index}`,
            ),
          }
          : response.body,
      })),
    },
    graphql: spec.graphql == null ? spec.graphql : (() => {
      const substituted = substituteValues(
        spec.graphql,
        declared,
        values,
        `${label} graphql`,
      )
      if (userAgent == null) return v.parse(GraphQLFixtureSchema, substituted)
      // Only a reviewed candidate view passes a User-Agent. There, and only
      // there, empty groups mean the derived zero-request fixture.
      const fixture = record(substituted) &&
          Array.isArray(substituted.groups) && substituted.groups.length === 0
        ? v.parse(ZeroRequestCandidateGraphQLSchema, substituted)
        : v.parse(GraphQLFixtureSchema, substituted)
      return rewriteUserAgent(fixture, userAgent)
    })(),
  }
}
