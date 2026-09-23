// Filesystem confinement canary. Runs through the Bubblewrap wrapper as a
// child of the runner and prints one JSON line; the runner decides pass/fail
// (preflight.ts). It touches no network so it can also run outside the lane.
const [marker, secondMarker, socketPath, allowedFile, escapeLink] = Deno.args
const out: Record<string, unknown> = {}
const decoder = new TextDecoder()

function describe(error: unknown): string {
  return error instanceof Error
    ? `${error.name}: ${error.message}`
    : String(error)
}

async function attempt(
  name: string,
  action: () => Promise<string>,
): Promise<void> {
  try {
    out[name] = await action()
  } catch (error) {
    out[name] = describe(error)
  }
}

const home = Deno.env.get("HOME") ?? ""
await attempt("allowedRead", () => Deno.readTextFile(allowedFile))
await attempt("tmpWrite", async () => {
  await Deno.writeTextFile("/tmp/fs-canary.txt", "tmp")
  return "ok"
})
await attempt("homeWrite", async () => {
  await Deno.writeTextFile(`${home}/fs-canary.txt`, "home")
  return "ok"
})
await attempt(
  "markerRead",
  async () => `READ ${await Deno.readTextFile(marker)}`,
)
await attempt("markerWrite", async () => {
  await Deno.writeTextFile(secondMarker, "leak")
  return "WROTE"
})
await attempt(
  "escapeLinkRead",
  async () => `READ ${await Deno.readTextFile(escapeLink)}`,
)
await attempt("socket", async () => {
  const conn = await Deno.connect({ transport: "unix", path: socketPath })
  conn.close()
  return "CONNECTED"
})
await attempt("rootWrite", async () => {
  await Deno.writeTextFile("/fs-canary-root", "leak")
  return "WROTE"
})
await attempt("hostHomeWrite", async () => {
  await Deno.mkdir("/home/fs-canary", { recursive: true })
  return "WROTE"
})
await attempt("usrLocalWrite", async () => {
  await Deno.writeTextFile("/usr/local/fs-canary", "leak")
  return "WROTE"
})
await attempt("usrLocalEntries", async () => {
  const names: string[] = []
  for await (const entry of Deno.readDir("/usr/local")) names.push(entry.name)
  return JSON.stringify(names)
})
for (
  const [name, path] of [
    ["runStat", "/run"],
    ["etcStat", "/etc"],
    ["sysStat", "/sys"],
    ["busStat", "/run/user/1000/bus"],
  ]
) {
  await attempt(name, async () => {
    await Deno.stat(path)
    return "PRESENT"
  })
}
await attempt("binShRealpath", () => Deno.realPath("/bin/sh"))
await attempt("procPids", async () => {
  const pids: number[] = []
  for await (const entry of Deno.readDir("/proc")) {
    if (/^\d+$/.test(entry.name)) pids.push(Number(entry.name))
  }
  return JSON.stringify(pids.sort((a, b) => a - b))
})
await attempt("procInit", async () => {
  const cmdline = await Deno.readFile("/proc/1/cmdline")
  return decoder.decode(cmdline).split("\0")[0]
})
await attempt("status", async () => {
  const status = await Deno.readTextFile("/proc/self/status")
  const pick = (key: string) =>
    status.split("\n").find((line) => line.startsWith(`${key}:`))?.slice(
      key.length + 1,
    ).trim() ?? "absent"
  return JSON.stringify({
    capEff: pick("CapEff"),
    capBnd: pick("CapBnd"),
    capPrm: pick("CapPrm"),
    noNewPrivs: pick("NoNewPrivs"),
    uid: pick("Uid"),
    gid: pick("Gid"),
  })
})
await attempt(
  "maxUserNamespaces",
  async () =>
    (await Deno.readTextFile("/proc/sys/user/max_user_namespaces")).trim(),
)
await attempt("usernsCreate", async () => {
  const result = await new Deno.Command("/usr/bin/unshare", {
    args: ["-U", "/bin/true"],
    env: { PATH: "/usr/bin:/bin" },
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  return result.success
    ? "CREATED"
    : `denied: ${decoder.decode(result.stderr).trim()}`
})
out.hostname = Deno.hostname()
out.pid = Deno.pid
out.uid = Deno.uid()
out.gid = Deno.gid()
out.cwd = Deno.cwd()
out.env = Object.keys(Deno.env.toObject()).sort()
console.log(JSON.stringify(out))
