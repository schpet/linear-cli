//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/strong.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::check_strong::check_strong,
};
use alloc::{format, string::ToString};
use markdown::{
    mdast::{Node, Strong},
    message::Message,
};

impl Handle for Strong {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let marker = check_strong(state)?;

        state.enter(ConstructName::Strong);

        let marks = marker.to_string().repeat(2);
        let between = state.container_phrasing(node, &Info::new(&marks, &marker.to_string()))?;
        let (between, surrounding) =
            crate::util::attention::encode(between, info.before, info.after, marker);
        let value = format!("{}{}{}", marks, between, marks);
        state.exit();
        state.attention = Some(surrounding);

        Ok(value)
    }
}

pub fn peek_strong(state: &State) -> char {
    state.options.strong
}
