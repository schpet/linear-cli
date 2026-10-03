//! `issue attach`: upload a file and link it in the issue's sidebar. Also the
//! file uploads `issue comment add --attach` embeds.
use std::path::Path;

use crate::cli::issue::IssueAttach;
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::commands::upload::{self, UploadedFile};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::upload::{
    AttachmentCreate, AttachmentCreateInput, AttachmentCreateVariables, CreatedAttachment,
    GetIssueId, GetIssueIdVariables,
};

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
pub(super) fn upload_file(ctx: &Ctx, path: &str, public: bool) -> Result<UploadedFile> {
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
async fn lookup(client: &LinearClient, identifier: &str) -> Result<String, Error> {
    let data: GetIssueId = client
        .query(GetIssueIdVariables {
            id: identifier.to_owned(),
        })
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    data.issue
        .map(|x| x.id.into_inner())
        .filter(|x| !x.is_empty())
        .ok_or_else(|| Error::not_found("Issue", identifier))
}
async fn attach(
    client: &LinearClient,
    issue_uuid: &str,
    file: &UploadedFile,
    title: Option<&str>,
    comment: Option<&str>,
) -> Result<CreatedAttachment, Error> {
    let data: AttachmentCreate = client
        .mutate(AttachmentCreateVariables {
            input: AttachmentCreateInput {
                issue_id: issue_uuid.to_owned(),
                title: title
                    .filter(|x| !x.is_empty())
                    .unwrap_or(&file.file.filename)
                    .to_owned(),
                url: file.asset_url.clone(),
                comment_body: comment.map(str::to_owned),
            },
        })
        .await?;
    if !data.attachment_create.success {
        return Err(Error::new("Linear did not create the attachment"));
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
    let mut output = outcome::done(
        "Attached",
        "file",
        &format!("{} to issue {identifier}", attachment.title),
        Some(&attachment.url),
    );
    if file.file.content_type.starts_with("image/") {
        output.push_str(&format!("Hint: Sidebar link attachments do not render images inline. For inline display, run: linear issue comment add {identifier} --attach {}{}\n",quote_shell(path),if file.file.public{" --public"}else{""}));
    }
    output.into_bytes()
}
