import { assertEquals, assertRejects } from "@std/assert"
import { withSpinner } from "../../src/utils/spinner.ts"

async function withInteractiveStdout<T>(fn: () => Promise<T>): Promise<T> {
  const originalIsTerminal = Deno.stdout.isTerminal
  const originalNoColor = Deno.env.get("NO_COLOR")

  Deno.stdout.isTerminal = () => true
  Deno.env.delete("NO_COLOR")

  try {
    return await fn()
  } finally {
    Deno.stdout.isTerminal = originalIsTerminal
    if (originalNoColor != null) {
      Deno.env.set("NO_COLOR", originalNoColor)
    } else {
      Deno.env.delete("NO_COLOR")
    }
  }
}

Deno.test("withSpinner - starts and stops spinner around successful work", async () => {
  const events: string[] = []

  const result = await withInteractiveStdout(() =>
    withSpinner(
      () => {
        events.push("operation")
        return "ok"
      },
      {
        createSpinner: () => ({
          start: () => events.push("start"),
          stop: () => events.push("stop"),
        }),
      },
    )
  )

  assertEquals(result, "ok")
  assertEquals(events, ["start", "operation", "stop"])
})

Deno.test("withSpinner - stops spinner when work throws", async () => {
  const events: string[] = []

  await assertRejects(
    () =>
      withInteractiveStdout(() =>
        withSpinner(
          () => {
            events.push("operation")
            throw new Error("boom")
          },
          {
            createSpinner: () => ({
              start: () => events.push("start"),
              stop: () => events.push("stop"),
            }),
          },
        )
      ),
    Error,
    "boom",
  )

  assertEquals(events, ["start", "operation", "stop"])
})

Deno.test("withSpinner - skips spinner creation when disabled", async () => {
  let spinnerCreated = false

  const result = await withSpinner(
    () => "ok",
    {
      enabled: false,
      createSpinner: () => {
        spinnerCreated = true
        return {
          start: () => {},
          stop: () => {},
        }
      },
    },
  )

  assertEquals(result, "ok")
  assertEquals(spinnerCreated, false)
})
