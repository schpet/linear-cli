import { assertEquals, assertThrows } from "@std/assert"
import { getIntrospectionQuery } from "graphql"
import {
  buildPinnedSchema,
  matchGraphQL,
  type OperationInput,
} from "./graphql-match.ts"

const schema = buildPinnedSchema(`
  input Filter { name: String, nested: Nested = {} }
  input Nested { active: Boolean }
  scalar JSONObject
  interface Node { id: ID! }
  type User implements Node { id: ID!, name: String }
  type Team implements Node { id: ID! }
  type Query { user(id: ID, filter: Filter, page: Int = 1, meta: JSONObject): User, users(filters: [Filter!]): [User!], node: Node, ping: String }
  type Mutation { first(value: String): Boolean, second(value: String): Boolean, ping: String }
`)

function matches(expected: OperationInput, actual: OperationInput): boolean {
  return matchGraphQL(expected, actual, schema).matches
}

Deno.test("renamed operations, variables, equivalent fragments, and literal arguments match", () => {
  const expected = {
    document:
      "query One($id: ID!) { account: user(id: $id) { ...Fields } } fragment Fields on User { id name }",
    variables: { id: "u1" },
  }
  const actual = {
    document: 'query Other { account: user(id: "u1") { name id } }',
  }
  assertEquals(matches(expected, actual), true)
  assertEquals(matches(actual, expected), true)
})

Deno.test("sibling aliases match mixed literal and variable sources in both directions", () => {
  const literalFirst = {
    document:
      'query($key: ID!) { a: user(id: "u1") { id } b: user(id: $key) { id } }',
    variables: { key: "u1" },
  }
  const variableFirst = {
    document:
      'query($other: ID!) { a: user(id: $other) { id } b: user(id: "u1") { id } }',
    variables: { other: "u1" },
  }
  assertEquals(matches(literalFirst, variableFirst), true)
  assertEquals(matches(variableFirst, literalFirst), true)
})

Deno.test("selected multi-operation names are not parity assertions", () => {
  assertEquals(
    matches(
      {
        document: "query A { user { id } } query B { user { name } }",
        operationName: "B",
      },
      {
        document: "query X { user { name } } query Y { user { id } }",
        operationName: "X",
      },
    ),
    true,
  )
})

Deno.test("field shape, alias, directive, and mutation order are material", () => {
  const base = {
    document:
      "query Q($show: Boolean!) { alias: user { id name @include(if: $show) } }",
    variables: { show: true },
  }
  assertEquals(
    matches(base, {
      document:
        "query Q($show: Boolean!) { user { id name @include(if: $show) } }",
      variables: { show: true },
    }),
    false,
  )
  assertEquals(
    matches(base, {
      document:
        "query Q($show: Boolean!) { alias: user { id name @skip(if: $show) } }",
      variables: { show: true },
    }),
    false,
  )
  assertEquals(
    matches(base, {
      document: "query Q($show: Boolean!) { alias: user { id } }",
      variables: { show: true },
    }),
    false,
  )
  assertEquals(
    matches({ document: "mutation { first second }" }, {
      document: "mutation { second first }",
    }),
    false,
  )
})

Deno.test("argument values, omission, defaults, null and nested presence are distinct", () => {
  assertEquals(
    matches(
      { document: "{ user(id: 123) { id } }" },
      {
        document: "query($id: ID) { user(id: $id) { id } }",
        variables: { id: "123" },
      },
    ),
    true,
  )
  assertEquals(
    matches(
      { document: "{ user { id } }" },
      { document: "query($p: Int) { user(page: $p) { id } }" },
    ),
    true,
  )
  assertEquals(
    matches({ document: '{ user(filter: {name: "A"}) { id } }' }, {
      document: '{ user(filter: {name: "B"}) { id } }',
    }),
    false,
  )
  assertEquals(
    matches({ document: "{ user { id } }" }, {
      document: "{ user(id: null) { id } }",
    }),
    false,
  )
  assertEquals(
    matches({ document: "{ user { id } }" }, {
      document: "{ user(page: 1) { id } }",
    }),
    false,
  )
  assertEquals(
    matches({ document: "query($p: Int = 1) { user(page: $p) { id } }" }, {
      document: "{ user { id } }",
    }),
    false,
  )
  assertEquals(
    matches({ document: "{ user(filter: {nested: {}}) { id } }" }, {
      document: "{ user(filter: {nested: {active: null}}) { id } }",
    }),
    false,
  )
})

Deno.test("provided variable use and explicit literal origin are enforced", () => {
  const literal = { document: '{ user(id: "u1") { id } }' }
  const variable = {
    document: "query($key: ID) { user(id: $key) { id } }",
    variables: { key: "u1" },
  }
  assertEquals(matches(literal, variable), true)
  assertEquals(
    matches({ ...literal, exactOrigins: ["$.user.id"] }, variable),
    false,
  )
  assertEquals(
    matches(variable, {
      document: "query($key: ID) { user(id: $key) { id } }",
      variables: { key: "u2" },
    }),
    false,
  )
  assertEquals(
    matches(literal, {
      document: 'query($key: ID) { user(id: "u1") { id } }',
      variables: { key: "u1" },
    }),
    false,
  )
})

Deno.test("nested literal-variable origin and list input presence are preserved", () => {
  const expected = {
    document:
      "query($flag: Boolean) { user(filter: {nested: {active: $flag}}) { id } }",
    variables: { flag: true },
  }
  const literal = {
    document: "{ user(filter: {nested: {active: true}}) { id } }",
  }
  assertEquals(matches(expected, literal), true)
  assertEquals(
    matches(
      { ...expected, exactOrigins: ["$.user.filter.nested.active"] },
      literal,
    ),
    false,
  )
  assertEquals(
    matches(
      { document: '{ users(filters: [{name: "A"}]) { id } }' },
      {
        document:
          '{ users(filters: [{name: "A", nested: {active: null}}]) { id } }',
      },
    ),
    false,
  )
})

Deno.test("extra typename needs narrow opt-in", () => {
  const plain = { document: "{ user { id } }" }
  const extra = { document: "{ user { id __typename } }" }
  assertEquals(matches(plain, extra), false)
  assertEquals(matches({ ...plain, allowExtraTypename: true }, extra), true)
})

Deno.test("invalid fixture expectations throw; invalid actual operations mismatch", () => {
  assertThrows(
    () => matches({ document: "{ missing }" }, { document: "{ user { id } }" }),
    Error,
    "invalid fixture expectation",
  )
  assertEquals(
    matches({ document: "{ user { id } }" }, { document: "{ missing }" }),
    false,
  )
})

Deno.test("operation kind and singleton list contents are compared", () => {
  assertEquals(
    matches({ document: "query { ping }" }, { document: "mutation { ping }" }),
    false,
  )
  assertEquals(
    matches(
      { document: '{ users(filters: {name: "A"}) { id } }' },
      { document: '{ users(filters: {name: "B"}) { id } }' },
    ),
    false,
  )
  assertEquals(
    matches(
      { document: '{ users(filters: {name: "A"}) { id } }' },
      { document: '{ users(filters: [{name: "A"}]) { id } }' },
    ),
    true,
  )
})

Deno.test("fragment directives compose with field directives", () => {
  assertEquals(
    matches(
      {
        document:
          "{ user { ... @include(if: false) { name @include(if: true) } } }",
      },
      { document: "{ user { name @include(if: true) } }" },
    ),
    false,
  )
  assertEquals(
    matches(
      { document: "{ ping @include(if: true) }" },
      {
        document:
          "{ ...Frag @include(if: false) } fragment Frag on Query { ping @include(if: true) }",
      },
    ),
    false,
  )
})

Deno.test("directive exactOrigins paths distinguish field and fragment scopes", () => {
  const fieldLiteral = { document: "{ user { name @include(if: true) } }" }
  const fieldVariable = {
    document: "query($show: Boolean!) { user { name @include(if: $show) } }",
    variables: { show: true },
  }
  assertEquals(matches(fieldLiteral, fieldVariable), true)
  assertEquals(
    matches(
      { ...fieldLiteral, exactOrigins: ["$.user.name@include[0].if"] },
      fieldVariable,
    ),
    false,
  )

  const spreadVariable = {
    document:
      "query($show: Boolean!) { user { ...Names @include(if: $show) } } fragment Names on User { name }",
    variables: { show: true },
  }
  const spreadLiteral = {
    document:
      "{ user { ...Names @include(if: true) } } fragment Names on User { name }",
  }
  assertEquals(matches(spreadVariable, spreadLiteral), true)
  assertEquals(
    matches({
      ...spreadVariable,
      exactOrigins: ["$.user#spread[0]@include[0].if"],
    }, spreadLiteral),
    false,
  )
})

Deno.test("JSON data with an origin key keeps every field", () => {
  assertEquals(
    matches(
      { document: '{ user(meta: {origin: "web", n: 1}) { id } }' },
      { document: '{ user(meta: {origin: "web", n: 2}) { id } }' },
    ),
    false,
  )
  assertEquals(
    matches(
      {
        document: "query($meta: JSONObject) { user(meta: $meta) { id } }",
        variables: { meta: { origin: "web", n: 1 } },
      },
      {
        document: "query($meta: JSONObject) { user(meta: $meta) { id } }",
        variables: { meta: { origin: "web", n: 2 } },
      },
    ),
    false,
  )
})

Deno.test("introspection meta-fields match and distinguish arguments", () => {
  const introspection = getIntrospectionQuery()
  assertEquals(
    matches({ document: introspection }, { document: introspection }),
    true,
  )
  assertEquals(
    matches(
      { document: '{ __type(name: "User") { name } }' },
      { document: '{ __type(name: "Team") { name } }' },
    ),
    false,
  )
})

Deno.test("extra typename opt-in keeps aliased response fields and expected typename", () => {
  assertEquals(
    matches(
      { document: "{ ping }", allowExtraTypename: true },
      { document: "{ ping extra: __typename }" },
    ),
    false,
  )
  assertEquals(
    matches(
      { document: "{ user { id __typename } }", allowExtraTypename: true },
      { document: "{ user { id __typename } }" },
    ),
    true,
  )
})

Deno.test("nested type conditions preserve every concrete type restriction", () => {
  assertEquals(
    matches(
      { document: "{ node { ... on User { ... on Node { id } } } }" },
      { document: "{ node { ... on Team { ... on Node { id } } } }" },
    ),
    false,
  )
})
