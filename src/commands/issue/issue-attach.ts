import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { AttachmentCreateInput } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { uploadFile, validateFilePath } from "../../utils/upload.ts"
import { basename } from "@std/path"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  isClientError,
  isNotFoundError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

export const attachCommand = new Command()
  .name("attach")
  .description("Attach a file to an issue")
  .arguments("<issueId:string> <filepath:string>")
  .option("-t, --title <title:string>", "Custom title for the attachment")
  .option(
    "-c, --comment <body:string>",
    "Add a comment body linked to the attachment",
  )
  .action(async (options, issueId, filepath) => {
    const { title, comment } = options

    try {
      const resolvedIdentifier = await getIssueIdentifier(issueId)
      if (!resolvedIdentifier) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      // Validate file exists
      await validateFilePath(filepath)

      // Get the issue UUID (attachmentCreate needs UUID, not identifier)
      let issueUuid: string | undefined
      try {
        issueUuid = await getIssueId(resolvedIdentifier)
      } catch (error) {
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", resolvedIdentifier)
        }
        throw error
      }
      if (!issueUuid) {
        throw new NotFoundError("Issue", resolvedIdentifier)
      }

      // Upload the file
      const uploadResult = await uploadFile(filepath, {
        showProgress: shouldShowSpinner(),
      })
      console.log(`✓ Uploaded ${uploadResult.filename}`)

      // Create the attachment
      const mutation = gql(`
        mutation AttachmentCreate($input: AttachmentCreateInput!) {
          attachmentCreate(input: $input) {
            success
            attachment {
              id
              url
              title
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const attachmentTitle = title || basename(filepath)

      const input: AttachmentCreateInput = {
        issueId: issueUuid,
        title: attachmentTitle,
        url: uploadResult.assetUrl,
        commentBody: comment,
      }

      const data = await client.request(mutation, { input })

      if (!data.attachmentCreate.success) {
        throw new CliError("Failed to create attachment")
      }

      const attachment = data.attachmentCreate.attachment
      console.log(`✓ Attachment created: ${attachment.title}`)
      console.log(attachment.url)
    } catch (error) {
      handleError(error, "Failed to attach file")
    }
  })
