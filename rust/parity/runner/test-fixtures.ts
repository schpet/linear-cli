/** Shared case fixture for runner tests; this module does not register tests. */
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
