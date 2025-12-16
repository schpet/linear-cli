import { assertEquals, assertStringIncludes } from "@std/assert"
import { tokenCommand } from "../../../src/commands/auth/auth-token.ts"

Deno.test("auth token - masks token by default", async () => {
  Deno.env.set("LINEAR_API_KEY", "lin_api_test_token_1234567890")

  try {
    const output: string[] = []
    const originalLog = console.log
    const originalError = console.error

    console.log = (msg: string) => {
      output.push(msg)
    }
    console.error = (msg: string) => {
      output.push(msg)
    }

    try {
      await tokenCommand.parse([])
    } catch {
      // May exit, that's fine
    }

    console.log = originalLog
    console.error = originalError

    // Should have masked output
    const logOutput = output.join("\n")
    assertEquals(logOutput.includes("lin_api_test_token_1234567890"), false)
    assertStringIncludes(logOutput, "lin_api_")
    assertStringIncludes(logOutput, "*")
    assertStringIncludes(logOutput, "masked")
  } finally {
    Deno.env.delete("LINEAR_API_KEY")
  }
})

Deno.test("auth token - shows full token with --show flag", async () => {
  Deno.env.set("LINEAR_API_KEY", "lin_api_test_token_1234567890")

  try {
    const output: string[] = []
    const originalLog = console.log
    const originalError = console.error

    console.log = (msg: string) => {
      output.push(msg)
    }
    console.error = (msg: string) => {
      output.push(msg)
    }

    try {
      await tokenCommand.parse(["--show"])
    } catch {
      // May exit, that's fine
    }

    console.log = originalLog
    console.error = originalError

    // Should have full token
    const logOutput = output.join("\n")
    assertStringIncludes(logOutput, "lin_api_test_token_1234567890")
  } finally {
    Deno.env.delete("LINEAR_API_KEY")
  }
})

Deno.test("auth token - handles short tokens", async () => {
  Deno.env.set("LINEAR_API_KEY", "short")

  try {
    const output: string[] = []
    const originalLog = console.log
    const originalError = console.error

    console.log = (msg: string) => {
      output.push(msg)
    }
    console.error = (msg: string) => {
      output.push(msg)
    }

    try {
      await tokenCommand.parse([])
    } catch {
      // May exit, that's fine
    }

    console.log = originalLog
    console.error = originalError

    // Should mask short tokens completely
    const logOutput = output.join("\n")
    assertEquals(logOutput.includes("short"), false)
    assertStringIncludes(logOutput, "*")
  } finally {
    Deno.env.delete("LINEAR_API_KEY")
  }
})
