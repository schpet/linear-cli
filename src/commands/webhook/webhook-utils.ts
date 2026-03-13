import { ValidationError } from "../../utils/errors.ts"

export function parseWebhookResourceTypes(
  rawValue: string | undefined,
  options: { required: true },
): string[]
export function parseWebhookResourceTypes(
  rawValue: string | undefined,
  options?: { required?: boolean },
): string[] | undefined
export function parseWebhookResourceTypes(
  rawValue: string | undefined,
  options: { required?: boolean } = {},
): string[] | undefined {
  if (rawValue == null) {
    if (options.required) {
      throw new ValidationError("Webhook resource types are required", {
        suggestion:
          "Use --resource-types with a comma-separated list like Issue,Comment",
      })
    }
    return undefined
  }

  const resourceTypes = [
    ...new Set(
      rawValue
        .split(",")
        .map((value) => value.trim())
        .filter((value) => value.length > 0),
    ),
  ]

  if (resourceTypes.length === 0) {
    throw new ValidationError("Webhook resource types cannot be empty", {
      suggestion:
        "Use --resource-types with a comma-separated list like Issue,Comment",
    })
  }

  return resourceTypes
}

export function validateWebhookUrl(url: string): string {
  try {
    const parsed = new URL(url)
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      throw new ValidationError("Webhook URL must use http or https", {
        suggestion: "Use an absolute URL like https://example.com/webhooks",
      })
    }
    return url
  } catch (error) {
    if (error instanceof ValidationError) {
      throw error
    }
    throw new ValidationError("Webhook URL must be a valid absolute URL", {
      suggestion: "Use an absolute URL like https://example.com/webhooks",
    })
  }
}

export function getWebhookDisplayLabel(label?: string | null): string {
  if (label == null || label.trim().length === 0) {
    return "(unlabeled)"
  }
  return label
}

export function getWebhookScope(webhook: {
  allPublicTeams: boolean
  team?: { key: string; name: string } | null
}): string {
  if (webhook.allPublicTeams) {
    return "All public teams"
  }
  if (webhook.team != null) {
    return `${webhook.team.name} (${webhook.team.key})`
  }
  return "Multiple teams"
}

export function getWebhookStatus(webhook: {
  enabled: boolean
  archivedAt?: string | null
}): string {
  if (webhook.archivedAt != null) {
    return "archived"
  }
  return webhook.enabled ? "enabled" : "disabled"
}
