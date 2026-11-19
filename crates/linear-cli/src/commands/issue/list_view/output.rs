use std::collections::HashSet;
use std::time::SystemTime;

use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;

use crate::cli::issue::{IssueColumn, IssueListOutput};
use crate::commands::json;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::issue_read::ListedIssue;
use crate::refs::IssueReference;

pub(in crate::commands::issue) enum Output<'a> {
    Json,
    Format {
        name: &'a str,
        paging: bool,
    },
    Table {
        columns: &'a [IssueColumn],
        paging: bool,
    },
}

impl<'a> Output<'a> {
    pub(in crate::commands::issue) fn resolve(
        ctx: &Ctx,
        args: &'a IssueListOutput,
    ) -> Result<Self> {
        if args.json {
            return Ok(Self::Json);
        }
        let paging = !args.no_pager;
        if let Some(name) = &args.format {
            ctx.templates().validate_alias(name)?;
            return Ok(Self::Format { name, paging });
        }
        let mut columns = HashSet::new();
        for &column in &args.columns {
            if !columns.insert(column) {
                let name = column.to_possible_value().expect("all columns have names");
                return Err(Error::invalid(format!(
                    "--columns lists {} more than once",
                    name.get_name()
                ))
                .with_hint("Specify each column once, in the order it should be shown."));
            }
        }
        Ok(Self::Table {
            columns: &args.columns,
            paging,
        })
    }

    pub(in crate::commands::issue) fn print<R: Serialize + Into<ListedIssue>>(
        self,
        ctx: &Ctx,
        rows: Vec<R>,
        team: bool,
        assignee: bool,
    ) -> Result<()> {
        match self {
            Self::Json => ctx.print(json::render(&rows)),
            Self::Format { name, paging } => print_format(ctx, name, &rows, paging),
            Self::Table { columns, paging } => {
                let rows: Vec<_> = rows.into_iter().map(Into::into).collect();
                let layout = super::TableLayout {
                    team,
                    assignee,
                    columns,
                };
                let table = super::table(
                    ctx.templates(),
                    &rows,
                    layout,
                    SystemTime::now(),
                    ctx.color(),
                )?;
                super::print_table(ctx, &table, paging)
            }
        }
    }
}

fn print_format(ctx: &Ctx, name: &str, rows: &[impl Serialize], paging: bool) -> Result<()> {
    #[derive(Serialize)]
    struct RowContext {
        issue: Value,
    }

    if rows.is_empty() {
        return Ok(());
    }
    let styled = ctx.color();
    let mut rendered = String::new();
    for row in rows {
        let mut issue = serde_json::to_value(row).expect("issue rows serialize");
        let fields = issue.as_object_mut().expect("issue rows are objects");
        let identifier = fields
            .get("identifier")
            .and_then(Value::as_str)
            .expect("issue rows have an identifier");
        let url = fields
            .get("url")
            .and_then(Value::as_str)
            .expect("issue rows have a URL string");
        let reference = IssueReference::new(identifier, Some(url));
        fields.insert(
            "workspace_url_key".to_owned(),
            Value::from(reference.workspace_url_key),
        );

        let row = ctx
            .templates()
            .render_display(name, &RowContext { issue }, styled)?;
        if styled {
            rendered.push_str(&row.terminal());
        } else {
            rendered.push_str(&row.plain());
        }
        rendered.push('\n');
    }
    ctx.page(&rendered, paging)
}
