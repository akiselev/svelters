use rquickjs::HasRefs;
use smallvec::SmallVec;

use crate::inner::*;

pub type Idx = RegisteryKey;

use crate::inner::*;

pub trait INode {
    fn next<'js>(&self, ctx: Ctx<'js>) -> Option<Object<'js>>;
    fn prev<'js>(&self, ctx: Ctx<'js>) -> Option<Object<'js>>;
    fn parent<'js>(&self, ctx: Ctx<'js>) -> Option<Object<'js>>;
    fn children<'js>(&self, ctx: Ctx<'js>) -> &[Object<'js>];
    fn node_name<'js>(&self, ctx: Ctx<'js>) -> String;
    fn get_root_node<'js>(&self, ctx: Ctx<'js>) -> Option<Object<'js>>;
    fn append_child<'js>(&self, ctx: Ctx<'js>, node: Object<'js>) -> Result<()>;
    fn remove_child<'js>(&self, ctx: Ctx<'js>, node: Object<'js>) -> Result<()>;
    fn insert_before<'js>(&self, ctx: Ctx<'js>, node: Object<'js>, anchor: Object<'js>) -> Result<()>;
}

pub(in crate) struct NodeData<'js> {
    pub(in crate) next: Option<Object<'js>>,
    pub(in crate) prev: Option<Object<'js>>,
    pub(in crate) parent: Option<Object<'js>>,
    pub(in crate) children: Option<Vec<Object<'js>>>,
}

impl<'js> NodeData<'js> {
    pub(in crate) fn new() -> Self {
        Self {
            next: None,
            prev: None,
            parent: None,
            children: None,
        }
    }
}

impl<'js> HasRefs for NodeData<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        if let Some(next) = &self.next { next.mark(marker)}
        if let Some(prev) = &self.prev { prev.mark(marker)}
        if let Some(parent) = &self.parent { parent.mark(marker)}
        if let Some(children) = &self.children { children.iter().for_each(|child| child.mark(marker)) }
    }
}
