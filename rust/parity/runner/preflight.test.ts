import { assertThrows } from "@std/assert"
import {
  assertFilesystemDenials,
  assertNetworkDenials,
  PreflightError,
} from "./preflight.ts"

const filesystem: Record<string, unknown> = {
  markerRead: "NotFound: No such file or directory (os error 2)",
  markerWrite: "NotFound: No such file or directory (os error 2)",
  escapeLinkRead: "NotFound: No such file or directory (os error 2)",
  socket: "NotFound: No such file or directory (os error 2)",
  rootWrite: "PermissionDenied: Read-only file system (os error 30)",
  hostHomeWrite: "PermissionDenied: Read-only file system (os error 30)",
  usrLocalWrite: "PermissionDenied: Read-only file system (os error 30)",
  runStat: "NotFound: No such file or directory (os error 2)",
  etcStat: "NotFound: No such file or directory (os error 2)",
  sysStat: "NotFound: No such file or directory (os error 2)",
  busStat: "NotFound: No such file or directory (os error 2)",
  usernsCreate: "denied: unshare failed: No space left on device",
}

Deno.test("filesystem preflight rejects missing, malformed and positive denial fields", () => {
  assertFilesystemDenials(filesystem)
  for (const name of Object.keys(filesystem)) {
    for (
      const value of [undefined, null, 0, "", "WROTE", "denied", "Error: bogus"]
    ) {
      const changed = { ...filesystem, [name]: value }
      assertThrows(
        () => assertFilesystemDenials(changed),
        PreflightError,
        name === "usernsCreate" ? "nested user namespace" : name,
      )
    }
  }
})

const network: Record<string, unknown> = {
  outbound: "NetworkUnreachable: Network is unreachable (os error 101)",
  dns: "Error: proto error: io error: No such file or directory",
  fetch: "TypeError: dns error: failed to lookup address information",
}

Deno.test("network preflight requires explicit denial results for every field", () => {
  assertNetworkDenials(network)
  for (const name of Object.keys(network)) {
    for (
      const value of [
        undefined,
        null,
        0,
        "",
        "CONNECTED",
        "RESOLVED",
        "RESPONDED",
        "Error: bogus",
      ]
    ) {
      assertThrows(
        () => assertNetworkDenials({ ...network, [name]: value }),
        PreflightError,
        name === "outbound" ? "outbound TCP" : name,
      )
    }
  }
})
