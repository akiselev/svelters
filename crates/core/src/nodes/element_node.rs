use std::{cell::RefCell, rc::Rc};

use rquickjs::{Function, HasRefs, WithProto};
use smallvec::SmallVec;

use crate::inner::*;

#[derive(Clone)]
pub struct ElementNode<'js> {
    pub(in crate) rt: Rt,
    pub(in crate) idx: Idx,
    pub(in crate) typekey: TypeKey,
    pub(in crate) tag_name: &'static str,
    pub(in crate) class_name: &'static str,
    pub(in crate) inner: Inner<ElData<'js>>,
}

impl<'js> std::ops::Deref for ElementNode<'js> {
    type Target = Inner<ElData<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'js> PartialEq for ElementNode<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<'js> PartialEq<Node<'js>> for ElementNode<'js> {
    fn eq(&self, other: &Node<'js>) -> bool {
        match &other.this {
            UINode::Slot(_) => false,
            UINode::Text(_) => false,
            UINode::Element(node) => self.idx == node.idx,
        }
    }
}

impl<'js> PartialEq<SlotNode<'js>> for ElementNode<'js> {
    fn eq(&self, _other: &SlotNode<'js>) -> bool {
        false
    }
}

impl<'js, CLS> PartialEq<Virt<'js, CLS>> for ElementNode<'js> {
    fn eq(&self, other: &Virt<'js, CLS>) -> bool {
        self.idx == other.idx
    }
}

impl<'js> Eq for ElementNode<'js> {}

pub struct ElData<'js> {
    pub(in crate) node: NodeData<'js>,
    pub(in crate) dispatcher: Dispatcher<'js>,
}

impl<'js> HasRefs for ElementNode<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        let this = self.inner.borrow();
        this.node.mark_refs(marker)
    }
}

impl<'js> ElementNode<'js> {
    pub(in crate) fn new<CLS: Type + Clone>(
        rt: Rt,
        ctx: Ctx<'js>,
        tag_name: &'static str,
        cls: CLS,
    ) -> Result<Self> {
        let idx = rt.new_instance(cls);
        let dispatcher = Dispatcher::new();
        Ok(Self {
            rt,
            idx,
            typekey: Class::<CLS>::typekey(),
            tag_name,
            class_name: CLS::CLASS_NAME,
            inner: Inner::new(ElData {
                dispatcher,
                node: NodeData::new(),
            }),
        })
    }
}

mod js {
    use super::*;

    impl<'js> ClassDef for ElementNode<'js> {
        const CLASS_NAME: &'static str = "ElementNode";

        unsafe fn class_id() -> &'static mut ClassId {
            static mut CLASS_ID: ClassId = ClassId::new();
            &mut CLASS_ID
        }

        const HAS_PROTO: bool = true;

        const HAS_STATIC: bool = true;

        const HAS_REFS: bool = true;

        fn init_proto<'ctx>(_ctx: Ctx<'ctx>, _proto: &Object<'ctx>) -> Result<()> {
            Ok(())
        }

        fn init_static<'ctx>(_ctx: Ctx<'ctx>, _static: &Object<'ctx>) -> Result<()> {
            Ok(())
        }

        fn mark_refs(&self, marker: &RefsMarker) {
            HasRefs::mark_refs(self, marker)
        }

        fn into_js_obj<'ctx>(self, ctx: Ctx<'ctx>) -> Result<Value<'ctx>>
        where
            Self: Sized,
        {
            JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
        }

        fn from_js_ref<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_ref(ctx, &value)
        }

        fn from_js_mut<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<&'ctx mut Self>
        where
            Self: Sized,
        {
            let value = Object::from_js(ctx, value)?;
            JsClass::<Self>::try_mut(ctx, &value)
        }

        fn from_js_obj<'ctx>(ctx: Ctx<'ctx>, value: Value<'ctx>) -> Result<Self>
        where
            Self: Clone + Sized,
        {
            let value = Object::from_js(ctx, value)?;
            let instance = JsClass::<Self>::try_ref(ctx, &value)?;
            Ok(instance.clone())
        }
    }

    impl<'js> rquickjs::IntoJs<'js> for ElementNode<'js> {
        fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            <Self as rquickjs::ClassDef>::into_js_obj(self, ctx)
        }
    }

    impl<'a, 'js> rquickjs::FromJs<'js> for ElementNode<'js> {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ElementNode<'js> as rquickjs::ClassDef>::from_js_obj(ctx, value)
        }
    }

    impl<'a, 'js> rquickjs::FromJs<'js> for &'js ElementNode<'js> {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ElementNode<'js> as rquickjs::ClassDef>::from_js_ref(ctx, value)
        }
    }

    impl<'a, 'js> rquickjs::FromJs<'js> for &'js mut ElementNode<'js> {
        fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
            <ElementNode<'js> as rquickjs::ClassDef>::from_js_mut(ctx, value)
        }
    }
}
