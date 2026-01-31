/**
 * Bulk operation utilities for Linear CLI
 *
 * Provides common infrastructure for --bulk, --bulk-file, and --bulk-stdin flags
 * across multiple commands (initiative archive/delete, issue archive/delete, label delete, etc.)
 */

import { shouldShowSpinner } from "./hyperlink.ts"

/**
 * Result of a single bulk operation
 */
export interface BulkOperationResult {
  id: string
  name?: string
  success: boolean
  error?: string
}

/**
 * Summary of bulk operation execution
 */
export interface BulkOperationSummary {
  total: number
  succeeded: number
  failed: number
  results: BulkOperationResult[]
}

/**
 * Options for executing bulk operations
 */
export interface BulkExecutionOptions {
  /** Show progress during execution */
  showProgress?: boolean
  /** Enable colored output */
  colorEnabled?: boolean
  /** Skip confirmation prompt */
  force?: boolean
  /** Concurrency limit (default: 5) */
  concurrency?: number
}

/**
 * Read IDs from stdin (piped input)
 * Supports one ID per line or space-separated IDs
 */
export async function readIdsFromStdin(): Promise<string[]> {
  const decoder = new TextDecoder()
  const chunks: Uint8Array[] = []

  for await (const chunk of Deno.stdin.readable) {
    chunks.push(chunk)
  }

  // Concatenate all chunks into a single Uint8Array
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0)
  const combined = new Uint8Array(totalLength)
  let offset = 0
  for (const chunk of chunks) {
    combined.set(chunk, offset)
    offset += chunk.length
  }

  const input = decoder.decode(combined)
  return parseIds(input)
}

/**
 * Read IDs from a file
 * Supports one ID per line or space-separated IDs
 */
export async function readIdsFromFile(filePath: string): Promise<string[]> {
  try {
    const content = await Deno.readTextFile(filePath)
    return parseIds(content)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw new Error(`File not found: ${filePath}`)
    }
    throw error
  }
}

/**
 * Parse IDs from text input
 * Supports newline-separated, space-separated, or comma-separated IDs
 */
function parseIds(input: string): string[] {
  return input
    .split(/[\n\r,\s]+/)
    .map((id) => id.trim())
    .filter((id) => id.length > 0)
}

/**
 * Collect IDs from various sources (CLI args, file, stdin)
 */
export async function collectBulkIds(options: {
  /** IDs provided via --bulk flag */
  bulk?: string[]
  /** File path provided via --bulk-file flag */
  bulkFile?: string
  /** Whether --bulk-stdin flag was used */
  bulkStdin?: boolean
}): Promise<string[]> {
  const allIds: string[] = []

  // Collect from --bulk flag
  if (options.bulk && options.bulk.length > 0) {
    allIds.push(...options.bulk)
  }

  // Collect from --bulk-file
  if (options.bulkFile) {
    const fileIds = await readIdsFromFile(options.bulkFile)
    allIds.push(...fileIds)
  }

  // Collect from stdin
  if (options.bulkStdin) {
    const stdinIds = await readIdsFromStdin()
    allIds.push(...stdinIds)
  }

  // Deduplicate
  return [...new Set(allIds)]
}

/**
 * Execute bulk operations with progress reporting
 *
 * @param ids - Array of IDs to process
 * @param operation - Function that executes the operation for a single ID
 * @param options - Execution options
 * @returns Summary of all operations
 */
export async function executeBulkOperations<T extends BulkOperationResult>(
  ids: string[],
  operation: (id: string) => Promise<T>,
  options: BulkExecutionOptions = {},
): Promise<BulkOperationSummary> {
  const {
    showProgress = true,
    colorEnabled = true,
    concurrency = 5,
  } = options

  const results: T[] = []
  let completed = 0
  const total = ids.length

  // Process in batches for controlled concurrency
  const batches: string[][] = []
  for (let i = 0; i < ids.length; i += concurrency) {
    batches.push(ids.slice(i, i + concurrency))
  }

  const spinnerEnabled = shouldShowSpinner()

  // Progress display helper
  const updateProgress = () => {
    if (showProgress && spinnerEnabled) {
      const percent = Math.round((completed / total) * 100)
      const succeeded = results.filter((r) => r.success).length
      const failed = completed - succeeded
      const status = colorEnabled
        ? `\r⏳ Processing: ${completed}/${total} (${percent}%) - ✓ ${succeeded} ✗ ${failed}`
        : `\rProcessing: ${completed}/${total} (${percent}%) - OK: ${succeeded} Failed: ${failed}`
      Deno.stdout.writeSync(new TextEncoder().encode(status))
    }
  }

  // Process batches
  for (const batch of batches) {
    const batchResults = await Promise.all(
      batch.map(async (id) => {
        try {
          const result = await operation(id)
          completed++
          updateProgress()
          return result
        } catch (error) {
          completed++
          updateProgress()
          return {
            id,
            success: false,
            error: error instanceof Error ? error.message : String(error),
          } as T
        }
      }),
    )
    results.push(...batchResults)
  }

  // Clear progress line
  if (showProgress && spinnerEnabled) {
    Deno.stdout.writeSync(
      new TextEncoder().encode("\r" + " ".repeat(80) + "\r"),
    )
  }

  const succeeded = results.filter((r) => r.success).length

  return {
    total,
    succeeded,
    failed: total - succeeded,
    results,
  }
}

/**
 * Print bulk operation summary
 */
export function printBulkSummary(
  summary: BulkOperationSummary,
  options: {
    entityName: string
    operationName: string
    colorEnabled?: boolean
    showDetails?: boolean
  },
): void {
  const { entityName, operationName, colorEnabled = true, showDetails = true } =
    options

  console.log("")

  if (summary.failed === 0) {
    const msg =
      `✓ Successfully ${operationName} ${summary.succeeded} ${entityName}${
        summary.succeeded !== 1 ? "s" : ""
      }`
    console.log(colorEnabled ? msg : msg.replace("✓", "OK:"))
  } else if (summary.succeeded === 0) {
    const msg = `✗ Failed to ${
      operationName.replace(/ed$/, "")
    } all ${summary.total} ${entityName}${summary.total !== 1 ? "s" : ""}`
    console.log(colorEnabled ? msg : msg.replace("✗", "FAILED:"))
  } else {
    console.log(
      `Completed: ${summary.succeeded}/${summary.total} ${entityName}${
        summary.total !== 1 ? "s" : ""
      } ${operationName}`,
    )
    console.log(`  ✓ Succeeded: ${summary.succeeded}`)
    console.log(`  ✗ Failed: ${summary.failed}`)
  }

  // Show details for failures
  if (showDetails && summary.failed > 0) {
    console.log("\nFailed operations:")
    for (const result of summary.results) {
      if (!result.success) {
        const name = result.name ? ` (${result.name})` : ""
        console.log(
          `  - ${result.id}${name}: ${result.error || "Unknown error"}`,
        )
      }
    }
  }
}

/**
 * Check if bulk mode is requested based on options
 */
export function isBulkMode(options: {
  bulk?: string[]
  bulkFile?: string
  bulkStdin?: boolean
}): boolean {
  return Boolean(
    (options.bulk && options.bulk.length > 0) ||
      options.bulkFile ||
      options.bulkStdin,
  )
}
