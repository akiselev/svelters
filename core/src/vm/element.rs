use ouroboros::self_referencing;
use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct Element<'js> {
    pub(in crate) _ctx: Ctx<'js>,
    pub(in crate) node: ElementNode<'js>,
    pub(in crate) doc_cell: OnceCell<ViewModelRuntime>,
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
            node: self.node.clone(),
        })
    }

    pub fn document(&self) -> Result<&ViewModelRuntime> {
        self.doc_cell.get_or_try_init(|| {
            let doc: &ViewModelRuntime = self._ctx.globals().get("document")?;
            Result::Ok((*doc).clone())
        })
    }

    pub(in crate) fn dispatcher(&self) -> Result<Dispatcher<'js>> {
        Ok(self.node.borrow().dispatcher.clone())
    }
}

impl<'a, 'js> rquickjs::FromJs<'js> for Element<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let type_of = value.type_of();
        let node = Object::from_js(ctx, value)?;
        if node.instance_of::<ElementNode>() {
            Ok(Element {
                _ctx: ctx,
                node: ElementNode::from_js(ctx, node.clone().into_value())?,
                doc_cell: OnceCell::new(),
            })
        } else {
            Err(Error::new(
                ctx,
                format!("{} is not an Element node", type_of),
            ))
        }
    }
}
