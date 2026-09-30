import {
  buildSchema,
  execute,
  getNamedType,
  getOperationAST,
  type GraphQLResolveInfo,
  type GraphQLSchema,
  type GraphQLType,
  isEnumType,
  isInterfaceType,
  isListType,
  isNonNullType,
  isObjectType,
  isScalarType,
  isUnionType,
  Kind,
  parse,
  validate,
} from "graphql"
import * as v from "valibot"
import { decodeByteValue } from "./bytes.ts"
import { matchGraphQL } from "./graphql-match.ts"
import { withoutFrozenLabelTeamDeclaration } from "./frozen-label-name.ts"
import { FROZEN_USER_AGENT } from "./schema.ts"
import { GraphQLState } from "./graphql-state.ts"
import { assetResponse, matchAssetRequest } from "./http-assets.ts"
import { LaneScheduler } from "./lane-scheduler.ts"
import type {
  AssetStepSpec,
  GraphQLStepSpec,
  RuntimeGraphQLFixtureSpec,
  RuntimeInteractionSpec,
} from "./schema.ts"

const MAX_REQUEST_BYTES = 4 * 1024 * 1024
const MAX_ISSUES = 32
const RequestSchema = v.strictObject({
  query: v.pipe(v.string(), v.minLength(1)),
  variables: v.optional(v.record(v.string(), v.unknown())),
  operationName: v.optional(v.pipe(v.string(), v.minLength(1))),
})

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value) &&
    (Object.getPrototypeOf(value) === Object.prototype ||
      Object.getPrototypeOf(value) === null)
}

function isJson(value: unknown): boolean {
  if (
    value == null || typeof value === "string" || typeof value === "boolean"
  ) return true
  if (typeof value === "number") return Number.isFinite(value)
  if (Array.isArray(value)) return value.every(isJson)
  return isRecord(value) &&
    Object.entries(value).every(([key, entry]) =>
      key !== "__proto__" && isJson(entry)
    )
}

async function readBody(request: Request): Promise<Uint8Array> {
  const reader = request.body?.getReader()
  if (reader == null) return new Uint8Array()
  const chunks: Uint8Array[] = []
  let length = 0
  try {
    for (;;) {
      const next = await reader.read()
      if (next.done) break
      length += next.value.length
      if (length > MAX_REQUEST_BYTES) {
        throw new Error("request body exceeds 4 MiB")
      }
      chunks.push(next.value)
    }
  } finally {
    reader.releaseLock()
  }
  const result = new Uint8Array(length)
  let offset = 0
  for (const chunk of chunks) {
    result.set(chunk, offset)
    offset += chunk.length
  }
  return result
}

function parseRequest(bytes: Uint8Array): v.InferOutput<typeof RequestSchema> {
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
  const value: unknown = JSON.parse(text)
  const parsed = v.parse(RequestSchema, value)
  if (parsed.variables != null && !isJson(parsed.variables)) {
    throw new Error("variables must contain finite JSON values")
  }
  return parsed
}

function strictDate(value: string): boolean {
  return /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|[+-]\d\d:\d\d)$/.test(
    value,
  ) && strictDay(value.slice(0, 10)) &&
    Number(value.slice(11, 13)) <= 23 && Number(value.slice(14, 16)) <= 59 &&
    Number(value.slice(17, 19)) <= 59 && Number.isFinite(Date.parse(value))
}

function strictDay(value: string): boolean {
  return /^\d{4}-\d\d-\d\d$/.test(value) &&
    Number.isFinite(Date.parse(`${value}T00:00:00Z`)) &&
    new Date(`${value}T00:00:00Z`).toISOString().slice(0, 10) === value
}

function strictDuration(value: string): boolean {
  return /^P(?=.*\d)(?:\d+Y)?(?:\d+M)?(?:\d+W)?(?:\d+D)?(?:T(?:\d+H)?(?:\d+M)?(?:\d+(?:\.\d+)?S)?)?$/
    .test(value)
}

function validateScalar(value: unknown, name: string): boolean {
  switch (name) {
    case "String":
      return typeof value === "string"
    case "ID":
      return typeof value === "string"
    case "Boolean":
      return typeof value === "boolean"
    case "Int":
      return typeof value === "number" && Number.isInteger(value) &&
        value >= -2147483648 && value <= 2147483647
    case "Float":
      return typeof value === "number" && Number.isFinite(value)
    case "DateTime":
      return typeof value === "string" && strictDate(value)
    case "DateTimeOrDuration":
      return typeof value === "string" &&
        (strictDate(value) || strictDuration(value))
    case "Duration":
      return typeof value === "string" && strictDuration(value)
    case "JSON":
      if (typeof value !== "string") return false
      try {
        return isJson(JSON.parse(value))
      } catch {
        return false
      }
    case "JSONObject":
      return isRecord(value) && isJson(value)
    case "TimelessDate":
      return typeof value === "string" && strictDay(value)
    case "TimelessDateOrDuration":
      return typeof value === "string" &&
        (strictDay(value) || strictDuration(value))
    case "UUID":
      return typeof value === "string" &&
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          value,
        )
    default:
      throw new FixtureDataError(`unrecognized scalar ${name}`)
  }
}

export class FixtureDataError extends Error {}

function validateOutput(
  value: unknown,
  type: GraphQLType,
  schema: GraphQLSchema,
  state: GraphQLState,
): void {
  if (isNonNullType(type)) {
    if (value == null) throw new FixtureDataError("non-null field is null")
    validateOutput(value, type.ofType, schema, state)
    return
  }
  if (value == null) return
  if (isListType(type)) {
    if (!Array.isArray(value)) {
      throw new FixtureDataError("list field is not an array")
    }
    for (const element of value) {
      validateOutput(element, type.ofType, schema, state)
    }
    return
  }
  const named = getNamedType(type)
  if (isScalarType(named)) {
    if (!validateScalar(value, named.name)) {
      throw new FixtureDataError(`invalid ${named.name} value`)
    }
    return
  }
  if (isEnumType(named)) {
    if (
      typeof value !== "string" ||
      !named.getValues().some((entry) => entry.name === value)
    ) throw new FixtureDataError("invalid enum value")
    return
  }
  const record = state.resolve(value)
  if (!isRecord(record)) {
    throw new FixtureDataError("object field is not an object")
  }
  if (
    isObjectType(named) && record.__typename != null &&
    record.__typename !== named.name
  ) {
    throw new FixtureDataError(
      `concrete record __typename must be ${named.name}`,
    )
  }
  if (isUnionType(named) || isInterfaceType(named)) {
    const typename = record.__typename
    const concrete = typeof typename === "string"
      ? schema.getType(typename)
      : null
    if (
      concrete == null || !isObjectType(concrete) ||
      !schema.getPossibleTypes(named).includes(concrete)
    ) {
      throw new FixtureDataError(
        "abstract record needs a valid concrete __typename",
      )
    }
  }
}

/** Inspect only schema-typed composite sources; JSON scalar contents are opaque. */
function sourceHasSuccessFalse(
  source: unknown,
  type: GraphQLType,
  schema: GraphQLSchema,
  state: GraphQLState,
): boolean {
  if (isNonNullType(type)) {
    return sourceHasSuccessFalse(source, type.ofType, schema, state)
  }
  if (source == null) return false
  if (isListType(type)) {
    return Array.isArray(source) &&
      source.some((item) =>
        sourceHasSuccessFalse(item, type.ofType, schema, state)
      )
  }
  const named = getNamedType(type)
  if (!isObjectType(named) && !isInterfaceType(named) && !isUnionType(named)) {
    return false
  }
  const record = state.resolve(source)
  if (!isRecord(record)) return false
  const concrete = typeof record.__typename === "string"
    ? schema.getType(record.__typename)
    : named
  return (isObjectType(concrete) || isInterfaceType(concrete)) &&
    Object.hasOwn(concrete.getFields(), "success") && record.success === false
}

function jsonResponse(status: number, value: unknown): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  })
}
function failure(): Response {
  return jsonResponse(500, { errors: [{ message: "fixture mismatch" }] })
}

export async function projectGraphQLResponse(
  schema: GraphQLSchema,
  state: GraphQLState,
  step: Pick<GraphQLStepSpec, "response">,
  query: string,
  variables: Record<string, unknown> | undefined,
  operationName: string | undefined,
): Promise<
  { data: unknown; suppressEffects: boolean; mutationExecuted: boolean }
> {
  const source =
    step.response.kind === "data" || step.response.kind === "graphqlErrors"
      ? step.response.data
      : null
  if (source == null && step.response.kind === "graphqlErrors") {
    return { data: null, suppressEffects: false, mutationExecuted: false }
  }
  const fixtureErrors: string[] = []
  let suppressEffects = false
  let sawRootMutationField = false
  function fixtureError(message: string): never {
    fixtureErrors.push(message)
    throw new FixtureDataError(message)
  }
  const document = parse(query)
  const selected = getOperationAST(document, operationName)
  if (selected?.operation === "mutation" && isRecord(source)) {
    const mutationType = schema.getMutationType()
    for (const selection of selected.selectionSet.selections) {
      if (selection.kind !== Kind.FIELD || selection.directives?.length) {
        continue
      }
      const name = selection.name.value
      const field = mutationType?.getFields()[name]
      if (field == null || !Object.hasOwn(source, name)) continue
      try {
        if (sourceHasSuccessFalse(source[name], field.type, schema, state)) {
          suppressEffects = true
        }
      } catch (error) {
        fixtureError(
          error instanceof Error ? error.message : "invalid fixture value",
        )
      }
    }
  }
  const result = await execute({
    schema,
    document,
    rootValue: source,
    variableValues: variables,
    operationName,
    fieldResolver(
      rawSource: unknown,
      _args: Record<string, unknown>,
      _context: unknown,
      info: GraphQLResolveInfo,
    ) {
      if (info.parentType === schema.getMutationType()) {
        sawRootMutationField = true
      }
      let record: unknown
      try {
        record = state.resolve(rawSource)
      } catch {
        return fixtureError("record reference is missing")
      }
      if (!isRecord(record) || !Object.hasOwn(record, info.fieldName)) {
        return fixtureError(
          `missing fixture field ${info.parentType.name}.${info.fieldName}`,
        )
      }
      if (
        (isObjectType(info.parentType) || isInterfaceType(info.parentType)) &&
        Object.hasOwn(info.parentType.getFields(), "success") &&
        record.success === false
      ) suppressEffects = true
      const value = record[info.fieldName]
      try {
        validateOutput(value, info.returnType, schema, state)
        if (sourceHasSuccessFalse(value, info.returnType, schema, state)) {
          suppressEffects = true
        }
      } catch (error) {
        return fixtureError(
          error instanceof Error ? error.message : "invalid fixture value",
        )
      }
      return value
    },
    typeResolver(
      value: unknown,
      _context: unknown,
      _info: GraphQLResolveInfo,
      abstractType,
    ) {
      let record: unknown
      try {
        record = state.resolve(value)
      } catch {
        return fixtureError("record reference is missing")
      }
      if (!isRecord(record) || typeof record.__typename !== "string") {
        return fixtureError("abstract record lacks __typename")
      }
      const concrete = schema.getType(record.__typename)
      if (
        concrete == null || !isObjectType(concrete) ||
        !schema.getPossibleTypes(abstractType).includes(concrete)
      ) return fixtureError("abstract record has invalid __typename")
      return record.__typename
    },
  })
  if (fixtureErrors.length > 0) throw new FixtureDataError(fixtureErrors[0])
  if (result.errors != null && result.errors.length > 0) {
    throw new FixtureDataError(
      "GraphQL execution produced an unexpected error",
    )
  }
  return {
    data: result.data,
    suppressEffects,
    mutationExecuted: selected?.operation !== "mutation" ||
      sawRootMutationField,
  }
}

export interface GraphQLRequestSummary {
  kind: "graphql" | "asset"
  authorizationMatched: boolean
  userAgent: string | null
}

export interface GraphQLServer {
  port: number
  requests: GraphQLRequestSummary[]
  issues: readonly string[]
  consumed: number
  unexpected: number
  expectedGraphQL: number
  expectedAssets: number
  state: GraphQLState
  handleFixedHost(
    request: Request,
    host: NonNullable<AssetStepSpec["fixedHost"]>,
  ): Promise<Response>
  failFixture(reason: string): void
  stop(): Promise<void>
}

/** One listener for GraphQL and declared loopback assets, with ordered lane scheduling. */
export function startGraphQLServer(
  specFor: (port: number) => RuntimeGraphQLFixtureSpec,
  schema: GraphQLSchema,
): GraphQLServer {
  const requests: GraphQLRequestSummary[] = []
  const issues: string[] = []
  let unexpected = 0
  let resolved: {
    spec: RuntimeGraphQLFixtureSpec
    scheduler: LaneScheduler
    state: GraphQLState
  } | undefined
  function note(message: string): void {
    if (issues.length < MAX_ISSUES) issues.push(message.slice(0, 240))
  }
  function active(): NonNullable<typeof resolved> {
    if (resolved != null) return resolved
    const spec = specFor(server.addr.port)
    resolved = {
      spec,
      scheduler: new LaneScheduler(spec.groups, note),
      state: new GraphQLState(spec.initialRecords),
    }
    return resolved
  }
  async function handleInteraction(
    request: Request,
    origin: NonNullable<AssetStepSpec["fixedHost"]> | null,
  ): Promise<Response> {
    const current = active()
    const { scheduler, spec, state } = current
    const url = new URL(request.url)
    const path = `${url.pathname}${url.search}`
    const hostHeader = request.headers.get("host")
    const authorityMismatch = origin != null &&
      (url.protocol !== "https:" || url.hostname !== origin ||
        (url.port !== "" && url.port !== "443") ||
        (hostHeader != null &&
          hostHeader !== origin && hostHeader !== `${origin}:443`))
    let body: Uint8Array
    try {
      body = await readBody(request)
    } catch {
      scheduler.fail("request body is invalid or exceeds 4 MiB")
      requests.push({
        kind: request.method === "POST" ? "graphql" : "asset",
        authorizationMatched: false,
        userAgent: request.headers.get("user-agent"),
      })
      return failure()
    }
    let parsed: v.InferOutput<typeof RequestSchema> | null = null
    if (request.method === "POST" && path === spec.path) {
      try {
        parsed = parseRequest(body)
      } catch {
        scheduler.fail("GraphQL request has malformed JSON envelope")
      }
    }
    let mismatchReason: string | null = null
    const matches = (step: RuntimeInteractionSpec): boolean => {
      if (authorityMismatch) {
        mismatchReason = "fixed-host authority differs from CONNECT target"
        return false
      }
      if (step.kind === "asset") {
        if (step.fixedHost !== (origin ?? undefined)) return false
        const reason = matchAssetRequest(step, request, body)
        if (reason != null) mismatchReason = reason
        return reason == null
      }
      if (origin != null) return false
      if (parsed == null || request.method !== "POST" || path !== spec.path) {
        return false
      }
      if (
        !/^application\/json(?:\s*;\s*charset=utf-8)?$/i.test(
          request.headers.get("content-type") ?? "",
        )
      ) return false
      if (
        request.headers.get("authorization") !==
          step.identity.authorization ||
        request.headers.get("user-agent") !== step.identity.userAgent
      ) return false
      if (
        !Object.entries(step.identity.headers).every(([name, value]) =>
          request.headers.get(name) === value
        )
      ) return false
      if (
        Object.hasOwn(parsed, "variables") !==
          Object.hasOwn(step.operation, "variables")
      ) {
        mismatchReason = "GraphQL variables presence differs"
        return false
      }
      if (step.response.kind === "validationErrors") {
        return parsed.query.trim() === step.operation.document.trim()
      }
      const actual = {
        document: parsed.query,
        operationName: parsed.operationName,
        variables: parsed.variables,
      }
      const result = matchGraphQL(
        step.operation,
        step.identity.userAgent === FROZEN_USER_AGENT
          ? withoutFrozenLabelTeamDeclaration(actual)
          : actual,
        schema,
      )
      if (!result.matches) mismatchReason = result.reason
      return result.matches
    }
    const wasFinished = scheduler.finished
    const priorFailure = scheduler.failure
    const claim = scheduler.claim(matches)
    const step = claim?.step
    if (wasFinished) unexpected++
    requests.push({
      kind: step?.kind ?? (request.method === "POST" ? "graphql" : "asset"),
      authorizationMatched: step?.kind === "graphql"
        ? request.headers.get("authorization") === step.identity.authorization
        : step?.kind === "asset"
        ? !Object.entries(step.requiredHeaders).some(([name, value]) =>
          name.toLowerCase() === "authorization" &&
          request.headers.get(name) !== value
        )
        : false,
      userAgent: request.headers.get("user-agent"),
    })
    if (claim == null || step == null) {
      if (priorFailure != null) {
        note("request arrived after prior fixture failure")
      }
      if (mismatchReason != null && !wasFinished) note(mismatchReason)
      return failure()
    }
    const onAbort = () =>
      scheduler.fail("client disconnected during lane barrier")
    request.signal.addEventListener("abort", onAbort, { once: true })
    let ready: boolean
    try {
      ready = await claim.ready
    } finally {
      request.signal.removeEventListener("abort", onAbort)
    }
    if (!ready || scheduler.failure != null) return failure()
    try {
      let response: Response
      if (step.kind === "asset") {
        response = assetResponse(step)
      } else {
        if (parsed == null) {
          throw new FixtureDataError("GraphQL request envelope is missing")
        }
        if (step.response.kind === "validationErrors") {
          let errors: readonly { message: string }[]
          try {
            errors = validate(schema, parse(parsed.query))
          } catch (error) {
            errors = [{
              message: error instanceof Error ? error.message : "parse error",
            }]
          }
          if (
            errors.length === 0 ||
            JSON.stringify(errors.map((error) => error.message)) !==
              JSON.stringify(
                step.response.errors.map((error) => error.message),
              )
          ) {
            throw new FixtureDataError("GraphQL validation errors differ")
          }
          response = jsonResponse(step.response.status, {
            errors: step.response.errors,
          })
        } else if (step.response.kind === "transport") {
          response = new Response(
            new Blob([new Uint8Array(decodeByteValue(step.response.body))]),
            { status: step.response.status, headers: step.response.headers },
          )
        } else {
          const projection = await projectGraphQLResponse(
            schema,
            state,
            step,
            parsed.query,
            parsed.variables,
            parsed.operationName,
          )
          response = step.response.kind === "data"
            ? jsonResponse(200, { data: projection.data })
            : jsonResponse(step.response.status, {
              data: projection.data,
              errors: step.response.errors,
            })
          if (
            step.effects.length > 0 &&
            (step.response.kind === "data" && projection.mutationExecuted &&
                !projection.suppressEffects ||
              step.response.kind === "graphqlErrors" &&
                step.partialEffects === true)
          ) state.apply(step.effects)
        }
      }
      scheduler.complete(claim)
      return response
    } catch (error) {
      scheduler.fail(
        `fixture response is invalid: ${
          error instanceof Error ? error.message : "unexpected response failure"
        }`,
      )
      return failure()
    }
  }
  const server = Deno.serve({
    hostname: "127.0.0.1",
    port: 0,
    onListen() {},
    handler(request) {
      return handleInteraction(request, null)
    },
  })
  return {
    port: server.addr.port,
    requests,
    get issues() {
      return issues
    },
    get consumed() {
      return active().scheduler.consumed
    },
    get unexpected() {
      return unexpected
    },
    get expectedGraphQL() {
      return active().spec.groups.flatMap((group) =>
        group.mode === "ordered"
          ? group.steps
          : group.lanes.flatMap((lane) => lane.steps)
      ).filter((step) => step.kind === "graphql").length
    },
    get expectedAssets() {
      return active().spec.groups.flatMap((group) =>
        group.mode === "ordered"
          ? group.steps
          : group.lanes.flatMap((lane) => lane.steps)
      ).filter((step) => step.kind === "asset").length
    },
    get state() {
      return active().state
    },
    handleFixedHost(request, host) {
      return handleInteraction(request, host)
    },
    failFixture(reason) {
      active().scheduler.fail(reason)
    },
    async stop() {
      active().scheduler.stop()
      let timeout: number | null = null
      try {
        await Promise.race([
          server.shutdown(),
          new Promise<never>((_resolve, reject) => {
            timeout = setTimeout(
              () =>
                reject(new Error("GraphQL fixture shutdown exceeded 900 ms")),
              900,
            )
          }),
        ])
      } finally {
        if (timeout != null) clearTimeout(timeout)
      }
    },
  }
}

let pinnedSchema: Promise<GraphQLSchema> | undefined
export function loadPinnedGraphQLSchema(): Promise<GraphQLSchema> {
  pinnedSchema ??= Deno.readTextFile(
    new URL("../../../graphql/schema.graphql", import.meta.url),
  ).then(buildSchema)
  return pinnedSchema
}
