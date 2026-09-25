pub mod clap_input;
pub mod clap_tree;
pub mod fish_completion;
mod generated;
pub mod parser;
pub mod render;
pub mod spelling;
pub(crate) mod suggest;

pub use generated::{ROUTES, Route};

/// Cliffy's built-in `-h, --help` description, shared by help and completions.
pub const HELP_FLAG_DESCRIPTION: &str = "Show this help.";
/// Cliffy's built-in root `-V, --version` description.
pub const VERSION_FLAG_DESCRIPTION: &str = "Show the version number for this program.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchAction {
    Root,
    AuthList,
    AuthWhoami,
    TeamList,
    UserList,
    TeamStates,
    TemplateList,
    CycleList,
    LabelList,
    Document,
    ParentPending,
    Markdown,
    TeamId,
    Completions,
    CompletionsBash,
    CompletionsFish,
    CompletionsZsh,
    CompletionsComplete,
    Unimplemented,
}

#[derive(Clone, Copy, Debug)]
pub struct ArgumentMeta {
    pub name: &'static str,
    pub type_name: &'static str,
    pub action: &'static str,
    pub optional: bool,
    pub variadic: bool,
    pub list: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionScope {
    Local,
    InheritedGlobal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionDefault {
    Absent,
    Null,
    Integer(i64),
    Strings(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeHandler {
    Enum(&'static [&'static str]),
    Variable,
}

#[derive(Clone, Copy, Debug)]
pub struct TypeMeta {
    pub name: &'static str,
    pub global: bool,
    pub override_existing: bool,
    pub handler: TypeHandler,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteKind {
    SourceLeaf,
    ParentRoute,
    GeneratedCompletionChild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentAction {
    NotApplicable,
    PendingSafeFixture,
}

#[derive(Clone, Copy, Debug)]
pub struct OptionMeta {
    pub scope: OptionScope,
    pub name: &'static str,
    pub flags: &'static [&'static str],
    pub description: &'static str,
    pub type_definition: &'static str,
    pub args: &'static [ArgumentMeta],
    pub default: OptionDefault,
    pub required: bool,
    pub collect: bool,
    pub hidden: bool,
    pub global: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ExampleMeta {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct RouteMeta {
    pub route: Route,
    pub path: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub hidden: bool,
    pub description: &'static str,
    pub usage: &'static str,
    pub args_definition: Option<&'static str>,
    pub kind: RouteKind,
    pub parent_action: ParentAction,
    pub children: &'static [&'static str],
    pub examples: &'static [ExampleMeta],
    pub arguments: &'static [ArgumentMeta],
    pub local_types: &'static [TypeMeta],
    pub local_options: &'static [OptionMeta],
    pub inherited_global_options: &'static [OptionMeta],
}

pub fn root() -> Option<&'static RouteMeta> {
    ROUTES.first()
}

pub fn resolve_child(parent: &RouteMeta, name: &str) -> Option<&'static RouteMeta> {
    parent.children.iter().find_map(|child| {
        let path = format!("{} {child}", parent.path);
        ROUTES.iter().find(|route| {
            route.path == path && (route.name == name || route.aliases.contains(&name))
        })
    })
}
