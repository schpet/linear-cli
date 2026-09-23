import { decodeBase64, encodeBase64 } from "@std/encoding/base64"

export type ByteValue = { utf8: string } | { base64: string }

const encoder = new TextEncoder()
const lossy = new TextDecoder()

export function decodeByteValue(value: ByteValue): Uint8Array {
  if ("utf8" in value) return encoder.encode(value.utf8)
  return decodeBase64(value.base64)
}

/** Prefer utf8 when the bytes round-trip exactly; otherwise base64. */
export function encodeByteValue(bytes: Uint8Array): ByteValue {
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
    if (bytesEqual(encoder.encode(text), bytes)) return { utf8: text }
  } catch {
    // fall through to base64
  }
  return { base64: encodeBase64(bytes) }
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(bytes))
  return Array.from(
    new Uint8Array(digest),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("")
}

export function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false
  return true
}

/** Index of the first differing byte, or -1 when equal. */
export function firstDifference(a: Uint8Array, b: Uint8Array): number {
  const shared = Math.min(a.length, b.length)
  for (let i = 0; i < shared; i++) if (a[i] !== b[i]) return i
  return a.length === b.length ? -1 : shared
}

/** Escaped, bounded excerpt around an offset for evidence files. */
export function excerpt(bytes: Uint8Array, at: number, radius = 48): string {
  const start = Math.max(0, at - radius)
  const end = Math.min(bytes.length, at + radius)
  const slice = lossy.decode(bytes.subarray(start, end))
  return `${start > 0 ? "…" : ""}${JSON.stringify(slice)}${
    end < bytes.length ? "…" : ""
  }`
}

export function concatBytes(chunks: Uint8Array[]): Uint8Array {
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0)
  const out = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    out.set(chunk, offset)
    offset += chunk.length
  }
  return out
}
