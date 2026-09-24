/** Shared case fixture for runner tests; this module does not register tests. */
import { join } from "@std/path"
import {
  buildStatusHelper,
  type StatusHelperArtifact,
} from "./helpers/build-status-helper.ts"

/**
 * Build the real status helper with the pinned recipe into a test-owned
 * directory under /var/tmp (never /tmp, which every sandbox binds over).
 * Tests fail closed without GCC; there is no test-only production fallback.
 */
export async function testStatusHelper(
  dir: string,
): Promise<StatusHelperArtifact> {
  return await buildStatusHelper({ stageDir: join(dir, "status-helper") })
}

export function validCase(): Record<string, unknown> {
  return {
    id: "sample",
    route: "linear",
    reason: "sample",
    argv: ["--version"],
    stdin: { utf8: "" },
    cwdFixture: "empty",
    env: {
      HOME: "{{home}}",
      XDG_CONFIG_HOME: "{{configHome}}",
      APPDATA: "{{configHome}}",
      PATH: "{{bin}}",
      DENO_DIR: "{{denoDir}}",
      LINEAR_IGNORE_ENV_FILE: "1",
    },
    substitutions: ["home", "configHome", "bin", "denoDir"],
    timeoutMs: 1000,
    outputCapBytes: 1024,
    fixtureServer: null,
    expected: {
      exit: { code: 0 },
      stdout: { utf8: "x" },
      stderr: { base64: "" },
      fileEffects: [],
    },
    deviation: null,
  }
}
