import { bytesEqual, decodeByteValue } from "./bytes.ts"
import type { AssetStepSpec } from "./schema.ts"

export function matchAssetRequest(
  step: AssetStepSpec,
  request: Request,
  body: Uint8Array,
): string | null {
  const url = new URL(request.url)
  if (
    request.method !== step.method ||
    `${url.pathname}${url.search}` !== step.path
  ) return "asset method or full path/query differs"
  for (const [name, value] of Object.entries(step.requiredHeaders)) {
    if (request.headers.get(name) !== value) {
      return `asset required header ${name} differs`
    }
  }
  for (const name of step.forbiddenHeaders) {
    if (request.headers.has(name)) {
      return `asset forbidden header ${name} is present`
    }
  }
  if (!bytesEqual(body, decodeByteValue(step.body))) {
    return "asset request body differs"
  }
  return null
}

export function assetResponse(step: AssetStepSpec): Response {
  const headers = new Headers(step.response.headers)
  if (step.response.location != null) {
    headers.set("Location", step.response.location)
  }
  return new Response(
    new Blob([new Uint8Array(decodeByteValue(step.response.body))]),
    {
      status: step.response.status,
      headers,
    },
  )
}
