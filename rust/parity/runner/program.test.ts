import { assertEquals } from "@std/assert"
import { invocationFor } from "./program.ts"

Deno.test("the interpreted reference is invoked with exactly the compiled reference's permissions and a frozen cached-only module cache", () => {
  assertEquals(
    invocationFor({
      kind: "interpreted-reference",
      workspace: "/ref",
      deno: "/opt/deno",
    }, ["issue", "mine", "--help"]),
    {
      executable: "/opt/deno",
      args: [
        "run",
        "--cached-only",
        "--frozen",
        "--allow-all",
        "--quiet",
        "--config",
        "/ref/deno.json",
        "/ref/src/main.ts",
        "issue",
        "mine",
        "--help",
      ],
    },
  )
  assertEquals(
    invocationFor({ kind: "executable", path: "/opt/linear" }, ["--version"]),
    { executable: "/opt/linear", args: ["--version"] },
  )
})
