import { Spinner } from "@std/cli/unstable-spinner"
import { shouldShowSpinner } from "./hyperlink.ts"

interface SpinnerController {
  start(): void
  stop(): void
}

export interface WithSpinnerOptions {
  enabled?: boolean
  message?: string
  createSpinner?: (message?: string) => SpinnerController
}

function createDefaultSpinner(message?: string): SpinnerController {
  return message != null ? new Spinner({ message }) : new Spinner()
}

export async function withSpinner<T>(
  operation: () => Promise<T> | T,
  options: WithSpinnerOptions = {},
): Promise<T> {
  const { enabled = true, message, createSpinner = createDefaultSpinner } =
    options

  const spinner = enabled && shouldShowSpinner() ? createSpinner(message) : null

  spinner?.start()

  try {
    return await operation()
  } finally {
    spinner?.stop()
  }
}
