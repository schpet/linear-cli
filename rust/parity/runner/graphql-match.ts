import {
  buildSchema,
  coerceInputValue,
  type DefinitionNode,
  type DirectiveNode,
  type DocumentNode,
  getNamedType,
  getOperationAST,
  getVariableValues,
  type GraphQLArgument,
  type GraphQLInputType,
  type GraphQLSchema,
  type GraphQLType,
  isCompositeType,
  isInputObjectType,
  isInterfaceType,
  isListType,
  isNonNullType,
  isObjectType,
  Kind,
  type OperationDefinitionNode,
  parse,
  SchemaMetaFieldDef,
  type SelectionSetNode,
  TypeMetaFieldDef,
  TypeNameMetaFieldDef,
  validate,
  type ValueNode,
} from "graphql"

export interface OperationInput {
  document: string
  operationName?: string
  variables?: Record<string, unknown>
  exactOrigins?: readonly string[]
  allowExtraTypename?: boolean
}

export interface MatchResult {
  matches: boolean
  reason: string | null
}

type ValueOrigin = "omitted" | "defaulted" | "null" | "provided"
interface NormalValue {
  value?: unknown
  origin: ValueOrigin
  source?: "literal" | "variable"
  defaultKind?: "variable" | "argument" | "input"
}
interface FieldShape {
  responseName: string
  name: string
  arguments: Record<string, NormalValue>
  directives: DirectiveShape[]
  children: FieldShape[]
  conditions: string[]
}
interface DirectiveShape {
  name: string
  arguments: Record<string, NormalValue>
}
interface OperationShape {
  kind: string
  fields: FieldShape[]
  bindings: Record<string, unknown[]>
}

const expandedValue = Symbol("expanded value")
const normalizedValue = Symbol("normalized value")
function brand(value: NormalValue): NormalValue {
  Object.defineProperty(value, normalizedValue, { value: true })
  return value
}
interface WrappedValue {
  [expandedValue]: NormalValue
}
function wrapped(value: NormalValue): WrappedValue {
  return { [expandedValue]: value }
}
function isWrapped(value: unknown): value is WrappedValue {
  return typeof value === "object" && value != null && expandedValue in value
}
function unwrapExpanded(value: unknown): unknown {
  if (isWrapped(value)) return unwrapExpanded(value[expandedValue].value)
  if (Array.isArray(value)) return value.map(unwrapExpanded)
  if (typeof value !== "object" || value == null) return value
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [key, unwrapExpanded(entry)]),
  )
}

const own = (value: object, key: string): boolean => Object.hasOwn(value, key)

function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`
  if (value != null && typeof value === "object") {
    return `{${
      Object.keys(value).sort().map((key) =>
        `${JSON.stringify(key)}:${stable(Reflect.get(value, key))}`
      ).join(",")
    }}`
  }
  return JSON.stringify(value)
}

function namedDefinitions(document: DocumentNode): Map<string, DefinitionNode> {
  const result = new Map<string, DefinitionNode>()
  for (const definition of document.definitions) {
    if (definition.kind === Kind.FRAGMENT_DEFINITION) {
      result.set(definition.name.value, definition)
    }
  }
  return result
}

function select(
  input: OperationInput,
): { document: DocumentNode; operation: OperationDefinitionNode } {
  const document = parse(input.document)
  const operation = getOperationAST(document, input.operationName)
  if (operation == null) {
    throw new Error("operationName does not select exactly one operation")
  }
  return { document, operation }
}

function expandValue(
  node: ValueNode,
  variables: Readonly<Record<string, unknown>>,
  variableDefaults: Readonly<Record<string, unknown>>,
  bindings: Record<string, unknown[]>,
  path: string,
): NormalValue {
  if (node.kind === Kind.VARIABLE) {
    const name = node.name.value
    if (!own(variables, name) && !own(variableDefaults, name)) {
      return { origin: "omitted" }
    }
    const value = own(variables, name)
      ? variables[name]
      : variableDefaults[name]
    if (own(variables, name)) (bindings[name] ??= []).push({ path, value })
    if (!own(variables, name)) {
      return {
        origin: "defaulted",
        value,
        source: "variable",
        defaultKind: "variable",
      }
    }
    return {
      origin: value === null ? "null" : "provided",
      value,
      source: "variable",
    }
  }
  switch (node.kind) {
    case Kind.NULL:
      return { origin: "null", value: null, source: "literal" }
    case Kind.INT:
      return {
        origin: "provided",
        value: Number(node.value),
        source: "literal",
      }
    case Kind.FLOAT:
      return {
        origin: "provided",
        value: Number(node.value),
        source: "literal",
      }
    case Kind.STRING:
    case Kind.ENUM:
      return { origin: "provided", value: node.value, source: "literal" }
    case Kind.BOOLEAN:
      return { origin: "provided", value: node.value, source: "literal" }
    case Kind.LIST:
      return {
        origin: "provided",
        value: node.values.map((item, index) =>
          wrapped(expandValue(
            item,
            variables,
            variableDefaults,
            bindings,
            `${path}[${index}]`,
          ))
        ),
        source: "literal",
      }
    case Kind.OBJECT: {
      const value: Record<string, unknown> = {}
      for (const field of node.fields) {
        value[field.name.value] = wrapped(expandValue(
          field.value,
          variables,
          variableDefaults,
          bindings,
          `${path}.${field.name.value}`,
        ))
      }
      return { origin: "provided", value, source: "literal" }
    }
  }
}

function normalizeInput(
  value: NormalValue,
  type: GraphQLInputType,
  path: string,
  origins: Record<string, NormalValue>,
): NormalValue {
  origins[path] = value
  if (value.origin === "omitted" || value.value == null) return brand(value)
  if (isNonNullType(type)) {
    return normalizeInput(value, type.ofType, path, origins)
  }
  if (isListType(type)) {
    const entries = Array.isArray(value.value) ? value.value : [value.value]
    return brand({
      ...value,
      value: entries.map((entry, index) => {
        const child: NormalValue = isWrapped(entry) ? entry[expandedValue] : {
          origin: entry === null ? "null" : "provided",
          value: entry,
          source: value.source,
        }
        return normalizeInput(child, type.ofType, `${path}[${index}]`, origins)
      }),
    })
  }
  const named = getNamedType(type)
  if (!isInputObjectType(named)) {
    return brand({
      ...value,
      value: coerceInputValue(unwrapExpanded(value.value), type),
    })
  }
  if (typeof value.value !== "object" || Array.isArray(value.value)) {
    return brand(value)
  }
  const input = value.value
  const normalized: Record<string, unknown> = {}
  for (const [key, field] of Object.entries(named.getFields())) {
    let child: NormalValue
    if (own(input, key)) {
      const raw = Reflect.get(input, key)
      child = isWrapped(raw) ? raw[expandedValue] : {
        origin: raw === null ? "null" : "provided",
        value: raw,
        source: value.source,
      }
      if (child.origin === "omitted" && field.defaultValue !== undefined) {
        child = {
          origin: "defaulted",
          value: field.defaultValue,
          defaultKind: "input",
        }
      }
    } else if (field.defaultValue !== undefined) {
      child = {
        origin: "defaulted",
        value: field.defaultValue,
        defaultKind: "input",
      }
    } else child = { origin: "omitted" }
    const result = normalizeInput(child, field.type, `${path}.${key}`, origins)
    normalized[key] = result
  }
  return brand({ ...value, value: normalized })
}

function argumentsOf(
  nodes: readonly { name: { value: string }; value: ValueNode }[] | undefined,
  definitions: readonly GraphQLArgument[],
  variables: Readonly<Record<string, unknown>>,
  defaults: Readonly<Record<string, unknown>>,
  bindings: Record<string, unknown[]>,
  path: string,
  origins: Record<string, NormalValue>,
): Record<string, NormalValue> {
  const found = new Map(nodes?.map((node) => [node.name.value, node.value]))
  const result: Record<string, NormalValue> = {}
  for (const definition of definitions) {
    const node = found.get(definition.name)
    let value: NormalValue = node == null
      ? definition.defaultValue === undefined ? { origin: "omitted" } : {
        origin: "defaulted",
        value: definition.defaultValue,
        defaultKind: "argument",
      }
      : expandValue(
        node,
        variables,
        defaults,
        bindings,
        `${path}.${definition.name}`,
      )
    if (value.origin === "omitted" && definition.defaultValue !== undefined) {
      value = {
        origin: "defaulted",
        value: definition.defaultValue,
        defaultKind: "argument",
      }
    }
    result[definition.name] = normalizeInput(
      value,
      definition.type,
      `${path}.${definition.name}`,
      origins,
    )
  }
  return result
}

function directiveShape(
  nodes: readonly DirectiveNode[] | undefined,
  schema: GraphQLSchema,
  variables: Readonly<Record<string, unknown>>,
  defaults: Readonly<Record<string, unknown>>,
  bindings: Record<string, unknown[]>,
  path: string,
  origins: Record<string, NormalValue>,
): DirectiveShape[] {
  const result: DirectiveShape[] = []
  for (const [index, node] of (nodes ?? []).entries()) {
    const directive = schema.getDirective(node.name.value)
    if (directive == null) {
      throw new Error(`unknown directive @${node.name.value}`)
    }
    const args = argumentsOf(
      node.arguments,
      directive.args,
      variables,
      defaults,
      bindings,
      `${path}@${node.name.value}[${index}]`,
      origins,
    )
    result.push({ name: node.name.value, arguments: args })
  }
  return result
}

function shape(
  input: OperationInput,
  schema: GraphQLSchema,
): { shape: OperationShape; origins: Record<string, NormalValue> } {
  const { document, operation } = select(input)
  const errors = validate(schema, document)
  if (errors.length > 0) {
    throw new Error(errors.map((error) => error.message).join("; "))
  }
  const supplied = input.variables ?? {}
  const coerced = getVariableValues(
    schema,
    operation.variableDefinitions ?? [],
    supplied,
  )
  if (coerced.errors != null) {
    throw new Error(coerced.errors.map((error) => error.message).join("; "))
  }
  const defaults: Record<string, unknown> = {}
  for (const definition of operation.variableDefinitions ?? []) {
    if (definition.defaultValue != null) {
      defaults[definition.variable.name.value] =
        expandValue(definition.defaultValue, {}, {}, {}, "default").value
    }
  }
  const bindings: Record<string, unknown[]> = {}
  const origins: Record<string, NormalValue> = {}
  const fragments = namedDefinitions(document)
  const root = operation.operation === "mutation"
    ? schema.getMutationType()
    : operation.operation === "subscription"
    ? schema.getSubscriptionType()
    : schema.getQueryType()
  if (root == null) throw new Error(`schema has no ${operation.operation} root`)
  function mergeFields(
    input: FieldShape[],
    preserveOrder: boolean,
  ): FieldShape[] {
    const merged = new Map<string, FieldShape>()
    for (const field of input) {
      const key = stable(stripOrigins({
        responseName: field.responseName,
        name: field.name,
        arguments: field.arguments,
        directives: field.directives,
        conditions: field.conditions,
      }))
      const previous = merged.get(key)
      if (previous == null) merged.set(key, field)
      else previous.children.push(...field.children)
    }
    const result = [...merged.values()].map((field) => ({
      ...field,
      children: mergeFields(field.children, false),
    }))
    if (!preserveOrder) {
      result.sort((a, b) =>
        stable(stripOrigins(a)).localeCompare(stable(stripOrigins(b)))
      )
    }
    return result
  }
  function fields(
    selection: SelectionSetNode,
    parent: GraphQLType,
    conditions: string[],
    prefix: string,
    inherited: DirectiveShape[] = [],
    visiting = new Set<string>(),
    preserveOrder = false,
  ): FieldShape[] {
    const result: FieldShape[] = []
    const named = getNamedType(parent)
    if (!isCompositeType(named)) {
      throw new Error(`selection on non-composite type ${String(named)}`)
    }
    for (const [selectionIndex, item] of selection.selections.entries()) {
      if (item.kind === Kind.FRAGMENT_SPREAD) {
        const fragment = fragments.get(item.name.value)
        if (fragment?.kind !== Kind.FRAGMENT_DEFINITION) {
          throw new Error(`missing fragment ${item.name.value}`)
        }
        if (visiting.has(item.name.value)) throw new Error("cyclic fragment")
        const next = new Set(visiting)
        next.add(item.name.value)
        const fragmentDirectives = directiveShape(
          item.directives,
          schema,
          supplied,
          defaults,
          bindings,
          `${prefix}#spread[${selectionIndex}]`,
          origins,
        )
        const conditioned = schema.getType(fragment.typeCondition.name.value)
        if (conditioned == null) {
          throw new Error(
            `unknown fragment type ${fragment.typeCondition.name.value}`,
          )
        }
        const nextConditions = fragment.typeCondition.name.value === named.name
          ? conditions
          : [...conditions, fragment.typeCondition.name.value]
        result.push(
          ...fields(
            fragment.selectionSet,
            conditioned,
            nextConditions,
            prefix,
            [...inherited, ...fragmentDirectives],
            next,
            preserveOrder,
          ),
        )
      } else if (item.kind === Kind.INLINE_FRAGMENT) {
        const fragmentDirectives = directiveShape(
          item.directives,
          schema,
          supplied,
          defaults,
          bindings,
          `${prefix}#inline[${selectionIndex}]`,
          origins,
        )
        const conditioned = item.typeCondition == null
          ? parent
          : schema.getType(item.typeCondition.name.value)
        if (conditioned == null) throw new Error("unknown inline fragment type")
        const nextConditions = item.typeCondition == null ||
            item.typeCondition.name.value === named.name
          ? conditions
          : [...conditions, item.typeCondition.name.value]
        result.push(
          ...fields(
            item.selectionSet,
            conditioned,
            nextConditions,
            prefix,
            [...inherited, ...fragmentDirectives],
            visiting,
            preserveOrder,
          ),
        )
      } else {
        const field = item
        const name = field.name.value
        const responseName = field.alias?.value ?? name
        const path = `${prefix}.${responseName}`
        const ordinaryField = isObjectType(named) || isInterfaceType(named)
          ? named.getFields()[name]
          : undefined
        const fieldDefinition = name === "__typename"
          ? TypeNameMetaFieldDef
          : named === schema.getQueryType() && name === "__schema"
          ? SchemaMetaFieldDef
          : named === schema.getQueryType() && name === "__type"
          ? TypeMetaFieldDef
          : ordinaryField
        if (fieldDefinition == null) {
          throw new Error(`unknown field ${named.name}.${name}`)
        }
        const fieldArgs = fieldDefinition?.args ?? []
        const args = argumentsOf(
          field.arguments,
          fieldArgs,
          supplied,
          defaults,
          bindings,
          path,
          origins,
        )
        const directives = [
          ...inherited,
          ...directiveShape(
            field.directives,
            schema,
            supplied,
            defaults,
            bindings,
            path,
            origins,
          ),
        ]
        const child = field.selectionSet == null ? [] : fields(
          field.selectionSet,
          fieldDefinition?.type ?? named,
          [],
          path,
          [],
          visiting,
        )
        result.push({
          responseName,
          name,
          arguments: args,
          directives,
          children: child,
          conditions,
        })
      }
    }
    return mergeFields(result, preserveOrder)
  }
  const result = fields(
    operation.selectionSet,
    root,
    [],
    "$",
    [],
    new Set(),
    operation.operation === "mutation",
  )
  for (const key of Object.keys(supplied)) {
    if (bindings[key]?.length == null) {
      throw new Error(`supplied variable $${key} is unused`)
    }
  }
  return {
    shape: { kind: operation.operation, fields: result, bindings },
    origins,
  }
}

function stripOrigins(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stripOrigins)
  if (value == null || typeof value !== "object") return value
  if (normalizedValue in value) {
    return {
      origin: Reflect.get(value, "origin"),
      value: stripOrigins(Reflect.get(value, "value")),
      defaultKind: Reflect.get(value, "defaultKind"),
    }
  }
  const result: Record<string, unknown> = {}
  for (const [key, entry] of Object.entries(value)) {
    result[key] = stripOrigins(entry)
  }
  return result
}

function withoutExtraTypename(
  expected: FieldShape[],
  actual: FieldShape[],
): FieldShape[] {
  return actual.filter((field) =>
    field.name !== "__typename" || field.responseName !== "__typename" ||
    expected.some((wanted) =>
      wanted.name === "__typename" && wanted.responseName === "__typename" &&
      stable(wanted.conditions) === stable(field.conditions)
    )
  ).map((field) => {
    const counterpart = expected.find((wanted) =>
      wanted.responseName === field.responseName &&
      wanted.name === field.name &&
      stable(wanted.conditions) === stable(field.conditions)
    )
    return {
      ...field,
      children: withoutExtraTypename(
        counterpart?.children ?? [],
        field.children,
      ),
    }
  })
}

/** Compare selected, schema-valid operations without binding parity to variable or operation names. */
export function matchGraphQL(
  expected: OperationInput,
  actual: OperationInput,
  schema: GraphQLSchema,
): MatchResult {
  let left: ReturnType<typeof shape>
  let right: ReturnType<typeof shape>
  try {
    left = shape(expected, schema)
  } catch (error) {
    throw new Error(
      `invalid fixture expectation: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
  }
  try {
    right = shape(actual, schema)
  } catch (error) {
    return {
      matches: false,
      reason: `invalid actual operation: ${
        error instanceof Error ? error.message : String(error)
      }`,
    }
  }
  const leftFields = left.shape.fields
  const rightFields = expected.allowExtraTypename
    ? withoutExtraTypename(leftFields, right.shape.fields)
    : right.shape.fields
  if (left.shape.kind !== right.shape.kind) {
    return { matches: false, reason: "operation kind differs" }
  }
  const leftComparable = stripOrigins(leftFields)
  const rightComparable = stripOrigins(rightFields)
  if (stable(leftComparable) !== stable(rightComparable)) {
    return {
      matches: false,
      reason: "operation fields, arguments, directives, or value origin differ",
    }
  }
  for (const path of expected.exactOrigins ?? []) {
    const wanted = left.origins[path]
    const received = right.origins[path]
    if (
      wanted == null || received == null || wanted.source !== received.source
    ) return { matches: false, reason: `explicit origin differs at ${path}` }
  }
  return { matches: true, reason: null }
}

export function buildPinnedSchema(sdl: string): GraphQLSchema {
  return buildSchema(sdl)
}
