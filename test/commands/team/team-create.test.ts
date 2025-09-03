import { assertEquals } from "@std/assert"
import { createCommand } from "../../../src/commands/team/team-create.ts"

Deno.test("team create command", async (t) => {
  await t.step("should be defined", () => {
    assertEquals(typeof createCommand, "object")
    assertEquals(createCommand.getName(), "create")
  })

  await t.step("should have correct description", () => {
    assertEquals(createCommand.getDescription(), "Create a linear team")
  })

  await t.step("should have expected options", () => {
    const options = createCommand.getOptions()
    const optionNames = options.map((opt) => opt.name)

    assertEquals(optionNames.includes("name"), true)
    assertEquals(optionNames.includes("description"), true)
    assertEquals(optionNames.includes("key"), true)
    assertEquals(optionNames.includes("private"), true)
    assertEquals(optionNames.includes("no-color"), true)
    assertEquals(optionNames.includes("no-interactive"), true)
  })
})
