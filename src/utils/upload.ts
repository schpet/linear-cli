import { gql } from "../__codegen__/gql.ts"
import { getGraphQLClient } from "./graphql.ts"
import { basename, extname } from "@std/path"
import { CliError, NotFoundError, ValidationError } from "./errors.ts"

/**
 * MIME type mapping for common file extensions
 */
const MIME_TYPES: Record<string, string> = {
  // Images
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".webp": "image/webp",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
  ".bmp": "image/bmp",
  ".tiff": "image/tiff",
  ".tif": "image/tiff",

  // Documents
  ".pdf": "application/pdf",
  ".doc": "application/msword",
  ".docx":
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  ".xls": "application/vnd.ms-excel",
  ".xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  ".ppt": "application/vnd.ms-powerpoint",
  ".pptx":
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",

  // Text
  ".txt": "text/plain",
  ".md": "text/markdown",
  ".markdown": "text/markdown",
  ".csv": "text/csv",
  ".tsv": "text/tab-separated-values",
  ".html": "text/html",
  ".htm": "text/html",
  ".css": "text/css",
  ".xml": "text/xml",

  // Code
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".ts": "text/typescript",
  ".tsx": "text/typescript",
  ".jsx": "text/javascript",
  ".json": "application/json",
  ".yaml": "text/yaml",
  ".yml": "text/yaml",
  ".toml": "text/toml",
  ".sh": "text/x-shellscript",
  ".bash": "text/x-shellscript",
  ".py": "text/x-python",
  ".rb": "text/x-ruby",
  ".go": "text/x-go",
  ".rs": "text/x-rust",
  ".java": "text/x-java",
  ".c": "text/x-c",
  ".cpp": "text/x-c++",
  ".h": "text/x-c",
  ".hpp": "text/x-c++",

  // Archives
  ".zip": "application/zip",
  ".tar": "application/x-tar",
  ".gz": "application/gzip",
  ".7z": "application/x-7z-compressed",
  ".rar": "application/vnd.rar",

  // Audio
  ".mp3": "audio/mpeg",
  ".wav": "audio/wav",
  ".ogg": "audio/ogg",
  ".m4a": "audio/mp4",

  // Video
  ".mp4": "video/mp4",
  ".webm": "video/webm",
  ".mov": "video/quicktime",
  ".avi": "video/x-msvideo",

  // Other
  ".wasm": "application/wasm",
}

/**
 * Maximum file size for uploads (100MB)
 */
const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * Get MIME type from file extension
 */
export function getMimeType(filepath: string): string {
  const ext = extname(filepath).toLowerCase()
  return MIME_TYPES[ext] || "application/octet-stream"
}

/**
 * Result of a successful file upload
 */
export interface UploadResult {
  /** The permanent URL where the file is accessible */
  assetUrl: string
  /** The original filename */
  filename: string
  /** The file size in bytes */
  size: number
  /** The MIME type of the file */
  contentType: string
}

/**
 * Options for file upload
 */
export interface UploadOptions {
  /** Make the file publicly accessible (only works for images, default: auto-detect) */
  makePublic?: boolean
  /** Show progress indicator */
  showProgress?: boolean
}

/**
 * Check if a file type can be uploaded as public
 * Linear only allows public uploads for images (excluding SVG)
 */
function canBePublic(contentType: string): boolean {
  const publicTypes = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/tiff",
  ]
  return publicTypes.includes(contentType)
}

/**
 * Upload a file to Linear's cloud storage
 *
 * This is a two-step process:
 * 1. Request a signed upload URL from Linear's GraphQL API
 * 2. Upload the file directly to the signed URL
 *
 * @param filepath - Path to the file to upload
 * @param options - Upload options
 * @returns The asset URL and file metadata
 */
export async function uploadFile(
  filepath: string,
  options: UploadOptions = {},
): Promise<UploadResult> {
  const { showProgress = false } = options

  // Read file and get metadata
  const fileInfo = await Deno.stat(filepath)
  if (!fileInfo.isFile) {
    throw new ValidationError(`Not a file: ${filepath}`, {
      suggestion: "Please provide a path to a valid file",
    })
  }

  const size = fileInfo.size
  if (size > MAX_FILE_SIZE) {
    throw new ValidationError(
      `File too large: ${(size / 1024 / 1024).toFixed(2)}MB exceeds limit of ${
        MAX_FILE_SIZE / 1024 / 1024
      }MB`,
      { suggestion: "Please upload a file smaller than 100MB" },
    )
  }

  const filename = basename(filepath)
  const contentType = getMimeType(filepath)

  // Step 1: Request signed upload URL
  const mutation = gql(`
    mutation FileUpload($contentType: String!, $filename: String!, $size: Int!, $makePublic: Boolean) {
      fileUpload(contentType: $contentType, filename: $filename, size: $size, makePublic: $makePublic) {
        success
        uploadFile {
          assetUrl
          uploadUrl
          headers {
            key
            value
          }
        }
      }
    }
  `)

  const client = getGraphQLClient()
  const { Spinner } = await import("@std/cli/unstable-spinner")
  const { shouldShowSpinner } = await import("./hyperlink.ts")
  const spinner = showProgress && shouldShowSpinner()
    ? new Spinner({ message: `Uploading ${filename}...` })
    : null
  spinner?.start()

  // Auto-detect makePublic based on file type (only images can be public)
  const makePublic = options.makePublic ?? canBePublic(contentType)

  try {
    const data = await client.request(mutation, {
      contentType,
      filename,
      size,
      makePublic,
    })

    if (!data.fileUpload.success || !data.fileUpload.uploadFile) {
      throw new CliError("Failed to get upload URL from Linear")
    }

    const { assetUrl, uploadUrl, headers } = data.fileUpload.uploadFile

    // Step 2: Upload file to signed URL
    const fileData = await Deno.readFile(filepath)

    // Build headers - start with Content-Type which is required by the signed URL
    const uploadHeaders: Record<string, string> = {
      "content-type": contentType,
    }

    // Add headers returned from Linear (may override content-type if provided)
    for (const header of headers) {
      uploadHeaders[header.key] = header.value
    }

    const response = await fetch(uploadUrl, {
      method: "PUT",
      headers: uploadHeaders,
      body: fileData,
    })

    if (!response.ok) {
      const errorText = await response.text()
      throw new CliError(
        `Failed to upload file: ${response.status} ${response.statusText} - ${errorText}`,
      )
    }

    spinner?.stop()

    return {
      assetUrl,
      filename,
      size,
      contentType,
    }
  } catch (error) {
    spinner?.stop()
    throw error
  }
}

/**
 * Upload multiple files to Linear's cloud storage
 *
 * @param filepaths - Array of file paths to upload
 * @param options - Upload options
 * @returns Array of upload results
 */
export async function uploadFiles(
  filepaths: string[],
  options: UploadOptions = {},
): Promise<UploadResult[]> {
  const results: UploadResult[] = []

  for (const filepath of filepaths) {
    const result = await uploadFile(filepath, options)
    results.push(result)
  }

  return results
}

/**
 * Check if a file exists and is readable
 */
export async function validateFilePath(filepath: string): Promise<void> {
  try {
    const info = await Deno.stat(filepath)
    if (!info.isFile) {
      throw new ValidationError(`Not a file: ${filepath}`, {
        suggestion: "Please provide a path to a valid file",
      })
    }
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw new NotFoundError("File", filepath)
    }
    throw error
  }
}

/**
 * Format an uploaded file as a markdown link
 */
export function formatAsMarkdownLink(result: UploadResult): string {
  const isImage = result.contentType.startsWith("image/")
  if (isImage) {
    return `![${result.filename}](${result.assetUrl})`
  }
  return `[${result.filename}](${result.assetUrl})`
}
