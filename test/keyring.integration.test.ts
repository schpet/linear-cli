import { assertEquals, assertRejects } from "@std/assert"
import {
  deletePassword,
  getPassword,
  isAvailable,
  setPassword,
} from "../src/keyring/index.ts"

async function isKeyringAvailable(): Promise<boolean> {
  // CI sets this so a probe that wrongly reports "unavailable" can never quietly
  // turn the keyring job into a no-op: the test runs and fails loudly instead.
  if (Deno.env.get("LINEAR_KEYRING_INTEGRATION") === "1") return true
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
  if (Deno.build.os === "darwin") {
    try {
      // show-keychain-info is read-only and exits 36 (errSecInteractionNotAllowed)
      // when the keychain is locked with no UI session to unlock it, which is the
      // state under ssh and most agent shells. Reads still succeed while locked
      // (find-generic-password returns 44), so only a write-capable probe like
      // this one detects it. Any other exit code means the keychain is usable
      // and a failure below is a real bug, so don't widen this to code !== 0.
      const { code } = await new Deno.Command("/usr/bin/security", {
        args: ["show-keychain-info"],
        stdout: "null",
        stderr: "null",
      }).output()
      return code !== 36
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

      if (Deno.build.os === "linux") {
        const dbusAddress = Deno.env.get("DBUS_SESSION_BUS_ADDRESS")
        Deno.env.set(
          "DBUS_SESSION_BUS_ADDRESS",
          `unix:path=/tmp/linear-cli-missing-dbus-${Date.now()}`,
        )
        try {
          assertEquals(await isAvailable(), true)
          await assertRejects(
            () => getPassword(TEST_ACCOUNT),
            Error,
            "secret-tool lookup failed",
          )
          await assertRejects(
            () => deletePassword(TEST_ACCOUNT),
            Error,
            "secret-tool clear failed",
          )
        } finally {
          if (dbusAddress == null) {
            Deno.env.delete("DBUS_SESSION_BUS_ADDRESS")
          } else {
            Deno.env.set("DBUS_SESSION_BUS_ADDRESS", dbusAddress)
          }
        }

        const path = Deno.env.get("PATH")
        Deno.env.set("PATH", "/nonexistent")
        try {
          assertEquals(await isAvailable(), false)
        } finally {
          if (path == null) {
            Deno.env.delete("PATH")
          } else {
            Deno.env.set("PATH", path)
          }
        }
      }
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
