import { assertEquals } from "@std/assert"
import {
  deletePassword,
  getPassword,
  setPassword,
} from "../src/keyring/index.ts"

const TEST_ACCOUNT = `linear-cli-integration-test-${Date.now()}`

Deno.test({
  name: "keyring integration - set, get, and delete round-trip",
  ignore: Deno.build.os === "windows",
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
