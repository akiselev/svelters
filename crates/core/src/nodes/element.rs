use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct Element<'js> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) el_node: Object<'js>,

    pub(in crate) doc_cell: OnceCell<Document>,
    pub(in crate) node_cell: OnceCell<ElementNode<'js>>,
    pub(in crate) dispatcher_cell: OnceCell<Dispatcher>,
}

impl<'js> Element<'js> {
    pub fn handle(&self) -> Result<ElementHandle> {
        Ok(ElementHandle {
            ctx: Context::from_ctx(self.ctx)?,
            node_id: self.ctx.register(self.el_node.clone().into_value()),
            doc_cell: OnceCell::new(),
            dispatcher_cell: OnceCell::new(),
        })
    }
}

pub struct ElementHandle {
    pub(in crate) ctx: Context,
    pub(in crate) node_id: RegisteryKey,
    pub(in crate) doc_cell: OnceCell<Document>,
    pub(in crate) dispatcher_cell: OnceCell<Dispatcher>,
}

impl ElementHandle {
    pub fn document(&self) -> Result<&Document> {
        self.doc_cell.get_or_try_init(|| {
            let doc = self.ctx.with(|ctx| {
                let doc: &Document = ctx.globals().get("document")?;
                Result::Ok((*doc).clone())
            })?;
            Result::Ok(doc)
        })
    }

    pub fn dispatcher(&self) -> Result<&Dispatcher> {
        self.dispatcher_cell.get_or_try_init(|| {
            let dispatcher = self.ctx.with(|ctx| {
                let value = ctx.get_register(self.node_id).unwrap();
                let node: &ElementNode = ElementNode::from_js_ref(ctx, value)?;
                Result::Ok(node.borrow().dispatcher.clone())
            })?;
            Result::Ok(dispatcher)
        })
    }
}
