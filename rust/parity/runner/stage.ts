// Stage the pinned interpreted reference's module cache outside the lane.
// Children receive this path as DENO_DIR; the host cache is never inherited.
import { join } from "@std/path"

export interface StageOptions {
  workspace: string
  denoPath: string
  stageRoot: string
  lockSha256: string
  denoVersion: string
  restage: boolean
}

export interface StageResult {
  denoDir: string
  reused: boolean
}

const MARKER = "staged.json"

export async function stageReference(
  options: StageOptions,
): Promise<StageResult> {
  const stageDir = join(options.stageRoot, options.lockSha256.slice(0, 16))
  const denoDir = join(stageDir, "deno")
  const marker = join(stageDir, MARKER)
  const expected = JSON.stringify({
    lockSha256: options.lockSha256,
    denoVersion: options.denoVersion,
    workspace: options.workspace,
  })
  if (!options.restage) {
    const existing = await Deno.readTextFile(marker).catch(() => null)
    if (existing === expected) return { denoDir, reused: true }
  }
  await Deno.remove(stageDir, { recursive: true }).catch(() => {})
  await Deno.mkdir(denoDir, { recursive: true })
  const home = join(stageDir, "home")
  await Deno.mkdir(home, { recursive: true })
  const result = await new Deno.Command(options.denoPath, {
    args: [
      "install",
      "--quiet",
      "--frozen",
      "--config",
      join(options.workspace, "deno.json"),
      "--entrypoint",
      join(options.workspace, "src/main.ts"),
    ],
    cwd: options.workspace,
    env: {
      PATH: "/usr/local/bin:/usr/bin:/bin",
      HOME: home,
      DENO_DIR: denoDir,
      DENO_NO_UPDATE_CHECK: "1",
    },
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new Error(
      `staging the reference module cache failed: ${
        new TextDecoder().decode(result.stderr)
      }`,
    )
  }
  await Deno.writeTextFile(marker, expected)
  return { denoDir, reused: false }
}
