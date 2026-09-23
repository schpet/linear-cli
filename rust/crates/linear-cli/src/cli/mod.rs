mod generated;

pub use generated::{ROUTES, Route};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchAction {
    Root,
    Document,
    ParentPending,
    Unimplemented,
}

#[derive(Clone, Copy, Debug)]
pub struct OptionMeta {
    pub scope: &'static str,
    pub name: &'static str,
    pub flags: &'static [&'static str],
    pub description: &'static str,
    pub type_definition: &'static str,
    pub args_json: &'static str,
    pub default_json: &'static str,
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
    pub kind: &'static str,
    pub parent_action: &'static str,
    pub children: &'static [&'static str],
    pub examples: &'static [ExampleMeta],
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
