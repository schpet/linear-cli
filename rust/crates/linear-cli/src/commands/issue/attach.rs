//! `issue attach`: upload a file and link it in the issue's sidebar. Also the
//! file uploads `issue comment add --attach` embeds.
use std::path::Path;

use crate::cli::issue::IssueAttach;
use crate::commands::upload::{self, UploadedFile};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::upload::{
    AttachmentCreate, AttachmentCreateInput, AttachmentCreateVariables, CreatedAttachment,
    GetIssueId, GetIssueIdVariables,
};
use crate::graphql::transport::GraphQlTransport;
use cynic::{MutationBuilder, QueryBuilder};
pub fn run(ctx: &Ctx, args: &IssueAttach) -> Result<()> {
    attach_file(ctx, args).context("Failed to attach file")
}

fn attach_file(ctx: &Ctx, args: &IssueAttach) -> Result<()> {
    let identifier = super::require(ctx, Some(&args.issue_id))?;
    upload::validate_file(Path::new(&args.filepath))?;
    let client = ctx.client()?;
    let issue_uuid = ctx.spin(true, lookup(client, &identifier))?;
    let file = upload_file(ctx, &args.filepath, args.public)?;
    let attachment = ctx.spin(
        true,
        attach(
            client,
            &issue_uuid,
            &file,
            args.title.as_deref(),
            args.comment.as_deref(),
        ),
    )?;
    ctx.print(attach_output(
        &attachment,
        &identifier,
        &args.filepath,
        &file,
    ))
}

/// Uploads one file, printing its result (and any warning) as soon as it is done.
pub(crate) fn upload_file(ctx: &Ctx, path: &str, public: bool) -> Result<UploadedFile> {
    let path = Path::new(path);
    let file = upload::prepare(path, public)?;
    let client = ctx.client()?;
    let message = format!("Uploading {}...", file.filename);
    let uploaded = ctx.spin_with(&message, upload::upload(client, path, file))?;
    ctx.print(upload::output(&uploaded))?;
    if let Some(warning) = upload::warning(&uploaded) {
        ctx.eprint(warning)?;
    }
    Ok(uploaded)
}
pub fn validate_comment_id(id: Option<&str>) -> Result<(), Error> {
    if let Some(id) = id {
        let bytes = id.as_bytes();
        if !(crate::refs::is_linear_uuid(id)
            && bytes.get(14) == Some(&b'4')
            && matches!(bytes.get(19), Some(b'8' | b'9' | b'a' | b'A' | b'b' | b'B')))
        {
            return Err(Error::new(format!("Invalid comment ID: {id}"))
                .with_hint("--id must be a v4 UUID, like 123e4567-e89b-42d3-a456-426614174000."));
        }
    }
    Ok(())
}
pub fn compose_body(text: Option<&str>, files: &[UploadedFile]) -> String {
    let links = files
        .iter()
        .map(upload::markdown)
        .collect::<Vec<_>>()
        .join("\n");
    [text.unwrap_or(""), &links]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
pub fn comment_output(identifier: &str, url: &str) -> Vec<u8> {
    format!("✓ Comment added to {identifier}\n{url}\n").into_bytes()
}
fn lookup_request(identifier: &str) -> GraphQlRequest<GetIssueIdVariables> {
    GraphQlRequest::with_variables(GetIssueId::build(GetIssueIdVariables {
        id: identifier.to_owned(),
    }))
}
async fn lookup(transport: &GraphQlTransport, identifier: &str) -> Result<String, Error> {
    let data: GetIssueId = transport
        .execute(&lookup_request(identifier))
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    data.issue
        .map(|x| x.id.into_inner())
        .filter(|x| !x.is_empty())
        .ok_or_else(|| Error::not_found("Issue", identifier))
}
fn attach_request(
    issue_uuid: &str,
    file: &UploadedFile,
    title: Option<&str>,
    comment: Option<&str>,
) -> GraphQlRequest<AttachmentCreateVariables> {
    GraphQlRequest::with_variables(AttachmentCreate::build(AttachmentCreateVariables {
        input: AttachmentCreateInput {
            issue_id: issue_uuid.to_owned(),
            title: title
                .filter(|x| !x.is_empty())
                .unwrap_or(&file.file.filename)
                .to_owned(),
            url: file.asset_url.clone(),
            comment_body: comment.map(str::to_owned),
        },
    }))
}
async fn attach(
    transport: &GraphQlTransport,
    issue_uuid: &str,
    file: &UploadedFile,
    title: Option<&str>,
    comment: Option<&str>,
) -> Result<CreatedAttachment, Error> {
    let data: AttachmentCreate = transport
        .execute(&attach_request(issue_uuid, file, title, comment))
        .await
        .map_err(Error::from)?;
    if !data.attachment_create.success {
        return Err(Error::new("Failed to create attachment"));
    }
    Ok(data.attachment_create.attachment)
}
fn quote_shell(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || b"_./:@%+=-".contains(&x))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
fn attach_output(
    attachment: &CreatedAttachment,
    identifier: &str,
    path: &str,
    file: &UploadedFile,
) -> Vec<u8> {
    let mut output = format!(
        "✓ Sidebar link attachment created: {}\n{}\n",
        attachment.title, attachment.url
    );
    if file.file.content_type.starts_with("image/") {
        output.push_str(&format!("Hint: Sidebar link attachments do not render images inline. For inline display, run: linear issue comment add {identifier} --attach {}{}\n",quote_shell(path),if file.file.public{" --public"}else{""}));
    }
    output.into_bytes()
}
