//! `linear template`: issue, project and document templates.
mod json;
mod list;
mod view;

pub mod scope;

use crate::cli::template::TemplateCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub use view::by_id as template_by_id;

pub fn run(ctx: &Ctx, command: &TemplateCommand) -> Result<()> {
    match command {
        TemplateCommand::List(args) => list::run(ctx, args),
        TemplateCommand::View(args) => view::run(ctx, args),
    }
}
