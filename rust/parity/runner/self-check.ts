// Harness self-check: identical-executable sanity plus deliberately broken
// candidates that must fail on specific surfaces. Wrappers are generated in
// a temporary directory inside the lane, so no machine paths are committed.
import { join } from "@std/path"
import type { LoadedCase } from "./cases.ts"
import type { Surface } from "./compare.ts"
import type { Program } from "./program.ts"
import {
  type Candidate,
  type CaseResult,
  type RunContext,
  runCorpus,
} from "./run.ts"

export interface Control {
  name: string
  description: string
  caseId: string
  /** Every listed surface must be reported for the control to count as caught. */
  expectSurfaces: Surface[]
  /** When set, some candidate mismatch detail must contain this text. */
  expectDetail?: string
  /** Expected candidate status; controls for not-implemented expect that status. */
  expectStatus: "fail" | "not-implemented"
  script?: string
  implementedRoutes?: (route: string) => string[]
  limits?: RunContext["limits"]
}

export interface ControlResult {
  name: string
  description: string
  caseId: string
  expectStatus: string
  expectSurfaces: Surface[]
  status: string
  surfaces: Surface[]
  caught: boolean
  detail: string
}

const MARKER = "candidate-invoked.marker"

export function controls(referenceBinary: string): Control[] {
  const ref = JSON.stringify(referenceBinary)
  return [
    {
      name: "wrong-exit-code",
      description: "candidate reproduces output but exits 3",
      caseId: "version",
      expectSurfaces: ["exit"],
      expectStatus: "fail",
      script: `${ref} "$@"\nexit 3\n`,
    },
    {
      name: "stdout-moved-to-stderr",
      description: "candidate writes its stdout to stderr",
      caseId: "help-root",
      expectSurfaces: ["stdout", "stderr"],
      expectStatus: "fail",
      script: `exec ${ref} "$@" 1>&2\n`,
    },
    {
      name: "altered-bytes",
      description: "candidate output differs in one byte",
      caseId: "version",
      expectSurfaces: ["stdout"],
      expectStatus: "fail",
      script: `${ref} "$@" | /usr/bin/tr 'r' 'R'\n`,
    },
    {
      name: "json-trailing-newline",
      description: "candidate appends a newline to the compact api JSON",
      caseId: "api-loopback-viewer-200",
      expectSurfaces: ["stdout"],
      expectStatus: "fail",
      script: `${ref} "$@"\ncode=$?\nprintf '\\n'\nexit $code\n`,
    },
    {
      name: "substitution-still-strict",
      description:
        "an unrelated trailing byte change is caught even though {{fixturePort}} is substituted in expected stdout",
      caseId: "api-loopback-port-echo-200",
      expectSurfaces: ["stdout"],
      expectStatus: "fail",
      script: `${ref} "$@"\ncode=$?\nprintf '\\n'\nexit $code\n`,
    },
    {
      name: "unintended-file-write",
      description: "candidate writes a file under HOME",
      caseId: "version",
      expectSurfaces: ["files"],
      expectStatus: "fail",
      script:
        `${ref} "$@"\ncode=$?\nprintf 'leak' > "$HOME/leak.txt"\nexit $code\n`,
    },
    {
      name: "hang-past-deadline",
      description:
        "candidate never exits; its sleeping grandchild is killed with the group",
      caseId: "version",
      expectSurfaces: ["timeout"],
      expectStatus: "fail",
      script: `${ref} "$@"\n/bin/sleep 300 &\nwait\n`,
      limits: { timeoutMs: 1500 },
    },
    {
      name: "output-flood",
      description:
        "candidate exceeds the output cap; reported as truncated, not timeout",
      caseId: "version",
      expectSurfaces: ["truncated"],
      expectStatus: "fail",
      script: `${ref} "$@"\n/usr/bin/head -c 3000000 /dev/zero\n`,
      limits: { outputCapBytes: 1024 * 1024, timeoutMs: 10000 },
    },
    {
      name: "escape-to-real-host",
      description:
        "candidate points at https://api.linear.app inside the lane; the request fails and the fixture sees nothing",
      caseId: "api-loopback-viewer-200",
      expectSurfaces: ["exit", "fixture"],
      expectStatus: "fail",
      script:
        `LINEAR_GRAPHQL_ENDPOINT=https://api.linear.app/graphql exec ${ref} "$@"\n`,
    },
    {
      name: "missing-authorization",
      description:
        "candidate drops the API key so the fixture sees no Authorization header",
      caseId: "api-loopback-viewer-200",
      expectSurfaces: ["exit", "stderr", "fixture"],
      expectStatus: "fail",
      script: `unset LINEAR_API_KEY\nexec ${ref} "$@"\n`,
    },
    {
      name: "graphql-wrong-variable",
      description:
        "candidate supplies a different issue id through the public api flag",
      caseId: "api-graphql-variable",
      expectSurfaces: ["fixture"],
      expectStatus: "fail",
      script: `exec ${ref} "$1" --variable id=ABC-2 "$4"\n`,
    },
    {
      name: "graphql-extra-variable",
      description:
        "candidate adds an unrequested variable to a viewer operation",
      caseId: "api-graphql-viewer",
      expectSurfaces: ["fixture"],
      expectStatus: "fail",
      script: `exec ${ref} "$1" --variable unexpected=1 "$2"\n`,
    },
    {
      name: "graphql-omitted-null",
      description:
        "candidate omits the explicit after:null required by the first paginated request",
      caseId: "api-graphql-paginate",
      expectSurfaces: ["fixture"],
      expectStatus: "fail",
      script: `exec ${ref} "$1" "$3"\n`,
    },
    {
      name: "signal-death-is-not-exit-143",
      description:
        "candidate reproduces output, then dies by SIGTERM; the authenticated status reports the signal, not bwrap's folded 143",
      caseId: "version",
      expectSurfaces: ["exit"],
      expectDetail: "got signal SIGTERM",
      expectStatus: "fail",
      script: `${ref} "$@"\nkill -TERM $$\n`,
    },
    {
      name: "sigpipe-death-is-not-exit-141",
      description:
        "candidate reproduces output, then dies by SIGPIPE; reported as signal SIGPIPE with outer 141",
      caseId: "version",
      expectSurfaces: ["exit"],
      expectDetail: "got signal SIGPIPE",
      expectStatus: "fail",
      script: `${ref} "$@"\nkill -PIPE $$\n`,
    },
    {
      name: "not-implemented-from-descriptor",
      description:
        "descriptor omits the route; the candidate program is never invoked",
      caseId: "version",
      expectSurfaces: [],
      expectStatus: "not-implemented",
      script: `printf 'invoked' > "$HOME/${MARKER}"\nexec ${ref} "$@"\n`,
      implementedRoutes: (route) => [`${route} not-this-one`],
    },
    {
      name: "claimed-implemented-but-failing",
      description:
        "descriptor claims the route while the program exits 3; must be fail, never not-implemented",
      caseId: "version",
      expectSurfaces: ["exit"],
      expectStatus: "fail",
      script: `${ref} "$@"\nexit 3\n`,
      implementedRoutes: (route) => [route],
    },
  ]
}

export interface SelfCheckResult {
  controls: ControlResult[]
  identicalExecutable: CaseResult[]
  ok: boolean
}

export async function runSelfCheck(
  cases: LoadedCase[],
  baseline: Program,
  referenceBinary: string,
  ctx: RunContext,
  log: (line: string) => void,
): Promise<SelfCheckResult> {
  const everyRoute = new Set(cases.map((loaded) => loaded.spec.route))
  log(
    "self-check: identical executable (candidate = baseline interpreted reference)",
  )
  const identical = await runCorpus(cases, baseline, {
    name: "identical interpreted reference",
    program: baseline,
    implementedRoutes: everyRoute,
  }, ctx)
  for (const result of identical) {
    log(`  ${result.status.padEnd(16)} ${result.id}`)
  }
  const identicalOk = identical.every((result) => result.status === "pass")

  const dir = await Deno.makeTempDir({
    dir: ctx.sandboxParent,
    prefix: "linear-parity-controls-",
  })
  const results: ControlResult[] = []
  try {
    for (const control of controls(referenceBinary)) {
      const loaded = cases.find((item) => item.spec.id === control.caseId)
      if (loaded == null) {
        results.push({
          ...summaryOf(control),
          status: "missing-case",
          surfaces: [],
          caught: false,
          detail: `case ${control.caseId} is not in the corpus`,
        })
        continue
      }
      const path = join(dir, `${control.name}.sh`)
      await Deno.writeTextFile(path, `#!/bin/sh\n${control.script ?? ""}`, {
        mode: 0o755,
      })
      const candidate: Candidate = {
        name: control.name,
        program: { kind: "executable", path },
        implementedRoutes: new Set(
          control.implementedRoutes?.(loaded.spec.route) ?? [loaded.spec.route],
        ),
      }
      const [result] = await runCorpus([loaded], baseline, candidate, {
        ...ctx,
        limits: control.limits,
      })
      const surfaces = [
        ...new Set(
          (result.candidate?.mismatches ?? []).map((mismatch) =>
            mismatch.surface
          ),
        ),
      ]
      const caught = result.status === control.expectStatus &&
        control.expectSurfaces.every((surface) => surfaces.includes(surface)) &&
        (control.expectDetail == null ||
          (result.candidate?.mismatches ?? []).some((mismatch) =>
            mismatch.detail.includes(control.expectDetail ?? "")
          )) &&
        (control.expectStatus !== "not-implemented" || result.candidate == null)
      const detail = result.status === "baseline-drift"
        ? `baseline drift: ${
          result.baseline.mismatches.map((m) => `${m.surface}: ${m.detail}`)
            .join("; ")
        }`
        : (result.candidate?.mismatches ?? []).map((m) =>
          `${m.surface}: ${m.detail}`
        ).join("; ")
      results.push({
        ...summaryOf(control),
        status: result.status,
        surfaces,
        caught,
        detail,
      })
      log(
        `  ${
          caught ? "caught " : "MISSED "
        } ${control.name}: ${result.status} [${surfaces.join(", ")}]`,
      )
    }
  } finally {
    await Deno.remove(dir, { recursive: true }).catch(() => {})
  }
  return {
    controls: results,
    identicalExecutable: identical,
    ok: identicalOk && results.every((result) => result.caught),
  }
}

function summaryOf(
  control: Control,
): Pick<
  ControlResult,
  "name" | "description" | "caseId" | "expectStatus" | "expectSurfaces"
> {
  return {
    name: control.name,
    description: control.description,
    caseId: control.caseId,
    expectStatus: control.expectStatus,
    expectSurfaces: control.expectSurfaces,
  }
}
