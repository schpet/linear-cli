const LINEAR_IDENTIFIER_RE = /^([a-zA-Z0-9]+)-([1-9][0-9]*)$/
const LINEAR_IDENTIFIER_IN_TEXT_RE = /\b([a-zA-Z0-9]+)-([1-9][0-9]*)\b/

export interface ParsedIssueIdentifier {
  identifier: string
  teamKey: string
  issueNumber: string
}

function buildParsedIssueIdentifier(
  teamKey: string,
  issueNumber: string,
): ParsedIssueIdentifier {
  const normalizedTeamKey = teamKey.toUpperCase()

  return {
    identifier: `${normalizedTeamKey}-${issueNumber}`,
    teamKey: normalizedTeamKey,
    issueNumber,
  }
}

export function parseIssueIdentifier(
  value: string,
): ParsedIssueIdentifier | undefined {
  const match = value.match(LINEAR_IDENTIFIER_RE)
  if (!match) {
    return undefined
  }

  const teamKey = match[1]
  const issueNumber = match[2]
  if (teamKey == null || issueNumber == null) {
    return undefined
  }

  return buildParsedIssueIdentifier(teamKey, issueNumber)
}

export function findIssueIdentifierInText(
  value: string,
): ParsedIssueIdentifier | undefined {
  const match = value.match(LINEAR_IDENTIFIER_IN_TEXT_RE)
  if (!match) {
    return undefined
  }

  const teamKey = match[1]
  const issueNumber = match[2]
  if (teamKey == null || issueNumber == null) {
    return undefined
  }

  return buildParsedIssueIdentifier(teamKey, issueNumber)
}

export function getTeamKeyFromIssueIdentifier(
  value: string,
): string | undefined {
  return parseIssueIdentifier(value)?.teamKey
}

export function normalizeIssueIdentifier(value: string): string | undefined {
  return parseIssueIdentifier(value)?.identifier
}
