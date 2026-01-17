import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { AttachmentCreateInput } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"
import { uploadFile, validateFilePath } from "../../utils/upload.ts"
import { basename } from "@std/path"

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
        console.error(getNoIssueFoundMessage())
        Deno.exit(1)
      }

      // Validate file exists
      await validateFilePath(filepath)

      // Get the issue UUID (attachmentCreate needs UUID, not identifier)
      const issueUuid = await getIssueId(resolvedIdentifier)
      if (!issueUuid) {
        console.error(`✗ Issue not found: ${resolvedIdentifier}`)
        Deno.exit(1)
      }

      // Upload the file
      const uploadResult = await uploadFile(filepath, {
        showProgress: Deno.stdout.isTerminal(),
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
        throw new Error("Failed to create attachment")
      }

      const attachment = data.attachmentCreate.attachment
      console.log(`✓ Attachment created: ${attachment.title}`)
      console.log(attachment.url)
    } catch (error) {
      console.error("✗ Failed to attach file", error)
      Deno.exit(1)
    }
  })
