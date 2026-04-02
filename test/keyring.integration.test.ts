import { assertEquals } from "@std/assert"
import {
  deletePassword,
  getPassword,
  setPassword,
} from "../src/keyring/index.ts"

async function isKeyringAvailable(): Promise<boolean> {
  if (Deno.build.os === "linux") {
    try {
      const cmd = new Deno.Command("secret-tool", {
        args: ["lookup", "service", "linear-cli-probe"],
        stdout: "null",
        stderr: "piped",
      })
      const { code, stderr } = await cmd.output()
      const err = new TextDecoder().decode(stderr).trim()
      if (err.includes("was not provided by any .service files")) return false
      return code === 0 || (code === 1 && err === "")
    } catch {
      return false
    }
  }
  return true
}

const TEST_ACCOUNT = `linear-cli-integration-test-${Date.now()}`

Deno.test({
  name: "keyring integration - set, get, and delete round-trip",
  sanitizeResources: Deno.build.os !== "windows",
  ignore: !(await isKeyringAvailable()),
  fn: async () => {
    try {
      assertEquals(await getPassword(TEST_ACCOUNT), null)

      await setPassword(TEST_ACCOUNT, "lin_api_test_secret")
      assertEquals(await getPassword(TEST_ACCOUNT), "lin_api_test_secret")

      await setPassword(TEST_ACCOUNT, "lin_api_updated_secret")
      assertEquals(await getPassword(TEST_ACCOUNT), "lin_api_updated_secret")

      await deletePassword(TEST_ACCOUNT)
      assertEquals(await getPassword(TEST_ACCOUNT), null)
    } finally {
      // Ensure cleanup even if assertions fail
      try {
        await deletePassword(TEST_ACCOUNT)
      } catch (error) {
        console.error(
          `Cleanup warning: ${
            error instanceof Error ? error.message : String(error)
          }`,
        )
      }
    }
  },
})
