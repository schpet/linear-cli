mod style;
mod table;

use crate::cli::{OptionDefault, OptionMeta, ROUTES, RouteMeta, TypeHandler};
use crate::error::{AppError, AppErrorKind};

fn invariant(message: &str) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

fn dedent(value: &str) -> String {
    let mut result = String::new();
    let mut indent = 0;
    for line in value.lines() {
        if !result.is_empty() || !line.trim().is_empty() {
            if result.is_empty() {
                indent = line.len() - line.trim_start().len();
                result.push_str(line.trim_start());
            } else {
                result.push('\n');
                result.push_str(line.get(indent..).unwrap_or(""));
            }
        }
    }
    result.trim_end().to_owned()
}

fn short_description(value: &str) -> &str {
    value.trim().split('\n').next().unwrap_or("").trim()
}

fn completion_description(value: &str, colors: bool) -> String {
    // Cliffy's generated completion command styles these three shell snippets
    // before HelpGenerator receives its description. The exported manifest
    // preserves their text but cannot carry the runtime ANSI wrappers.
    [
        "~/.bashrc",
        "source <(linear completions [shell])",
        "linear completions [shell] --help",
    ]
    .iter()
    .fold(value.to_owned(), |description, snippet| {
        description.replace(
            snippet,
            &style::dim(&style::italic(snippet, colors), colors),
        )
    })
}

fn highlight_token(token: &str, colors: bool) -> String {
    let (optional, interior) = if let Some(inner) = token
        .strip_prefix('[')
        .and_then(|part| part.strip_suffix(']'))
    {
        (true, inner)
    } else if let Some(inner) = token
        .strip_prefix('<')
        .and_then(|part| part.strip_suffix('>'))
    {
        (false, inner)
    } else {
        return token.to_owned();
    };
    let name = interior.split(':').next().unwrap_or("");
    let opening = if optional { "[" } else { "<" };
    let closing = if optional { "]" } else { ">" };
    format!(
        "{}{}{}",
        style::yellow(opening, colors),
        style::bright_magenta(name, colors),
        style::yellow(closing, colors)
    )
}

fn highlight_arguments(value: &str, colors: bool) -> String {
    value
        .split(' ')
        .map(|token| highlight_token(token, colors))
        .collect::<Vec<_>>()
        .join(" ")
}

fn label(name: &str, colors: bool) -> String {
    format!("\n{}\n\n", style::bold(&format!("{name}:"), colors))
}

fn option_hints(option: &OptionMeta, route: &RouteMeta, colors: bool) -> String {
    let mut hints = Vec::new();
    if option.required {
        hints.push(style::yellow("required", colors));
    }
    let default = match option.default {
        OptionDefault::Absent | OptionDefault::Null => None,
        OptionDefault::Integer(number) => Some(style::yellow(&number.to_string(), true)),
        OptionDefault::Strings(values) => Some(format!(
            "[ {} ]",
            values
                .iter()
                .map(|value| style::green(&format!("\"{value}\""), true))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    };
    if let Some(value) = default {
        hints.push(format!("{}{}", style::bold("Default: ", colors), value));
    }
    if let Some(argument) = option.args.first()
        && let Some(ty) = route
            .local_types
            .iter()
            .find(|ty| ty.name == argument.type_name)
        && let TypeHandler::Enum(values) = ty.handler
        && !values.is_empty()
    {
        let values = values
            .iter()
            .map(|value| style::green(&format!("\"{value}\""), true))
            .collect::<Vec<_>>()
            .join(", ");
        hints.push(format!("{}{values}", style::bold("Values: ", colors)));
    }
    if hints.is_empty() {
        String::new()
    } else {
        format!("({})", hints.join(", "))
    }
}

fn visible_children(route: &RouteMeta) -> Result<Vec<&'static RouteMeta>, AppError> {
    let mut children = Vec::new();
    for child in route.children {
        let path = format!("{} {child}", route.path);
        let found = ROUTES
            .iter()
            .find(|candidate| candidate.path == path)
            .ok_or_else(|| invariant("generated child route is missing"))?;
        if !found.hidden {
            children.push(found);
        }
    }
    Ok(children)
}

pub fn help(route: &RouteMeta, colors: bool, long: bool) -> Result<String, AppError> {
    let usage = if route.usage.is_empty() {
        route.path.to_owned()
    } else {
        format!(
            "{} {}",
            route.path,
            highlight_arguments(route.usage, colors)
        )
    };
    let header = vec![
        vec![
            style::bold("Usage:", colors),
            style::bright_magenta(&usage, colors),
        ],
        vec![
            style::bold("Version:", colors),
            style::yellow(env!("CARGO_PKG_VERSION"), colors),
        ],
    ];
    let mut output = format!(
        "\n{}\n",
        table::render(&header, &[usize::MAX, usize::MAX], &[1, 1], 0)?
    );
    if !route.description.is_empty() {
        output.push_str(&label("Description", colors));
        let description = if route.path == "linear completions" {
            completion_description(route.description, colors)
        } else {
            route.description.to_owned()
        };
        output.push_str(&table::render(
            &[vec![dedent(&description)]],
            &[140],
            &[1],
            2,
        )?);
        output.push('\n');
    }
    let mut option_rows = Vec::new();
    option_rows.push(vec![
        format!(
            "{}, {}",
            style::bright_blue("-h", colors),
            style::bright_blue("--help", colors)
        ),
        String::new(),
        style::red(&style::bold("-", colors), colors),
        "Show this help.".to_owned(),
        String::new(),
    ]);
    if route.path == "linear" {
        option_rows.push(vec![
            format!(
                "{}, {}",
                style::bright_blue("-V", colors),
                style::bright_blue("--version", colors)
            ),
            String::new(),
            style::red(&style::bold("-", colors), colors),
            "Show the version number for this program.".to_owned(),
            String::new(),
        ]);
    }
    for (option, flags) in super::spelling::effective_help_options(route) {
        if option.hidden {
            continue;
        }
        option_rows.push(vec![
            flags
                .iter()
                .map(|flag| style::bright_blue(flag, colors))
                .collect::<Vec<_>>()
                .join(", "),
            highlight_arguments(option.type_definition, colors),
            style::red(&style::bold("-", colors), colors),
            if long {
                dedent(option.description)
            } else {
                short_description(option.description).to_owned()
            },
            option_hints(option, route, colors),
        ]);
    }
    output.push_str(&label("Options", colors));
    let typed = option_rows
        .iter()
        .any(|row| row.get(1).is_some_and(|definition| !definition.is_empty()));
    if typed {
        output.push_str(&table::render(
            &option_rows,
            &[60, 60, 1, 80, 60],
            &[2, 2, 1, 2, 0],
            2,
        )?);
    } else {
        let mut rows = option_rows;
        for row in &mut rows {
            if row.len() != 5 {
                return Err(invariant("option help row has unexpected width"));
            }
            row.remove(1);
        }
        output.push_str(&table::render(&rows, &[60, 1, 80, 60], &[2, 1, 2, 0], 2)?);
    }
    output.push('\n');
    let children = visible_children(route)?;
    if !children.is_empty() {
        let typed = children
            .iter()
            .any(|child| child.args_definition.is_some_and(|value| !value.is_empty()));
        let rows = children
            .iter()
            .map(|child| {
                let names = std::iter::once(child.name)
                    .chain(child.aliases.iter().copied())
                    .map(|name| style::bright_blue(name, colors))
                    .collect::<Vec<_>>()
                    .join(", ");
                let separator = style::red(&style::bold("-", colors), colors);
                if typed {
                    vec![
                        names,
                        highlight_arguments(child.args_definition.unwrap_or(""), colors),
                        separator,
                        short_description(child.description).to_owned(),
                    ]
                } else {
                    vec![
                        names,
                        separator,
                        short_description(child.description).to_owned(),
                    ]
                }
            })
            .collect::<Vec<_>>();
        output.push_str(&label("Commands", colors));
        if typed {
            output.push_str(&table::render(&rows, &[60, 60, 1, 80], &[2, 2, 1, 2], 2)?);
        } else {
            output.push_str(&table::render(&rows, &[60, 1, 80], &[2, 1, 2], 2)?);
        }
        output.push('\n');
    }
    if !route.examples.is_empty() {
        let rows = route
            .examples
            .iter()
            .map(|example| {
                vec![
                    style::dim(&style::bold(example.name, colors), colors),
                    dedent(example.description),
                ]
            })
            .collect::<Vec<_>>();
        output.push_str(&label("Examples", colors));
        output.push_str(&table::render(&rows, &[150, 150], &[1, 1], 2)?);
        output.push('\n');
    }
    output.push('\n');
    Ok(output)
}

pub fn long_version(colors: bool) -> String {
    format!(
        "{} {}\n",
        style::bold("linear", colors),
        style::bright_blue(env!("CARGO_PKG_VERSION"), colors)
    )
}
