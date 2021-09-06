use ouroboros::self_referencing;
use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct Element<'js> {
    pub(in crate) _ctx: Ctx<'js>,
    pub(in crate) el_node: Object<'js>,

    pub(in crate) doc_cell: OnceCell<Document>,
    pub(in crate) node_cell: OnceCell<ElementNode<'js>>,
}

impl<'js> Element<'js> {
    pub fn ctx(&self) -> Ctx<'js> {
        self._ctx
    }

    pub fn event(&self, name: &'static str) -> Result<SubscriptionBuilder<'js>> {
        let callbacks = {
            let disp = self.dispatcher()?;
            let mut inner = disp.borrow_mut();
            inner
                .subs
                .entry(name)
                .or_insert_with(|| Callbacks::<'js>::new())
                .clone()
        };
        Ok(SubscriptionBuilder {
            name,
            ctx: self.ctx(),
            callbacks,
            element: self.clone(),
        })
    }

    pub fn document(&self) -> Result<&Document> {
        self.doc_cell.get_or_try_init(|| {
            let doc: &Document = self._ctx.globals().get("document")?;
            Result::Ok((*doc).clone())
        })
    }

    pub(in crate) fn node(&self) -> Result<&ElementNode<'js>> {
        self.node_cell.get_or_try_init(|| {
            ElementNode::from_js_obj(self._ctx, self.el_node.clone().into_value())
        })
    }

    pub(in crate) fn dispatcher(&self) -> Result<Dispatcher<'js>> {
        Ok(self.node()?.borrow().dispatcher.clone())
    }

    pub fn cast_as<CLS: Clone + Type>(&self) -> Result<Virt<'js, CLS>> {
        let func: Function = self
            .el_node
            .get("castAs")
            .expect("No castAs function found on Node");
        func.call((This(self.el_node.clone()), CLS::CLASS_NAME))
    }
}
