import { assertEquals, assertThrows } from "@std/assert"
import { FixtureStateError, GraphQLState } from "./graphql-state.ts"

Deno.test("GraphQL state resolves named records and applies ordered compare-and-swap effects", () => {
  const initial = { "Issue:i1": { id: "i1", title: "old" } }
  const state = new GraphQLState(initial)
  assertEquals(state.resolve({ $record: "Issue:i1" }), initial["Issue:i1"])
  state.apply([
    {
      kind: "put",
      record: "Issue:i1",
      before: { value: { id: "i1", title: "old" } },
      after: { id: "i1", title: "new" },
    },
    {
      kind: "put",
      record: "Issue:i2",
      before: { absent: true },
      after: { id: "i2" },
    },
    {
      kind: "delete",
      record: "Issue:i1",
      before: { value: { id: "i1", title: "new" } },
    },
  ])
  assertEquals(state.matches({ "Issue:i2": { id: "i2" } }), true)
  assertEquals(initial["Issue:i1"].title, "old")
})

Deno.test("a failed effect rolls back the entire step", () => {
  const state = new GraphQLState({ "Issue:i1": { id: "i1" } })
  assertThrows(
    () =>
      state.apply([
        {
          kind: "put",
          record: "Issue:i2",
          before: { absent: true },
          after: { id: "i2" },
        },
        {
          kind: "delete",
          record: "Issue:i1",
          before: { value: { id: "wrong" } },
        },
      ]),
    FixtureStateError,
    "prior value",
  )
  assertEquals(state.matches({ "Issue:i1": { id: "i1" } }), true)
  assertThrows(
    () => state.resolve({ $record: "Issue:missing" }),
    FixtureStateError,
    "missing",
  )
})
