use crate::inner::*;

pub struct Obj<FW: Framework, T: JsClass<FW>> {
    fw: FW,
    inner: T,
}

impl<FW: Framework, T: JsClass<FW>> ClassDef for Obj<FW, T> {
    const CLASS_NAME: &'static str = <T as JsClass<FW>>::CLASS_NAME;

    unsafe fn class_id() -> &'static mut ClassId {
        <T as JsClass<FW>>::class_id()
    }

    // With prototype
    const HAS_PROTO: bool = true;
    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        <T as JsClass<FW>>::Def::init_proto(ctx, proto)
    }

    // With statics
    const HAS_STATIC: bool = true;
    fn init_static<'js>(ctx: Ctx<'js>, ctor: &Object<'js>) -> Result<()> {
        <T as JsClass<FW>>::Def::init_static(ctx, ctor)
    }

    // With internal references
    const HAS_REFS: bool = true;
    fn mark_refs(&self, marker: &RefsMarker) {
        // marker.mark(&self.some_persistent_value);
    }
}
