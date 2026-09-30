//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/emphasis.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::check_emphasis::check_emphasis,
};
use alloc::{format, string::ToString};
use markdown::{
    mdast::{Emphasis, Node},
    message::Message,
};

impl Handle for Emphasis {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let marker = check_emphasis(state)?;

        state.enter(ConstructName::Emphasis);

        let marks = marker.to_string();
        let between = state.container_phrasing(node, &Info::new(&marks, &marker.to_string()))?;
        let (between, surrounding) =
            crate::util::attention::encode(between, info.before, info.after, marker);
        let value = format!("{}{}{}", marks, between, marks);
        state.exit();
        state.attention = Some(surrounding);

        Ok(value)
    }
}

pub fn peek_emphasis(state: &State) -> char {
    state.options.emphasis
}
