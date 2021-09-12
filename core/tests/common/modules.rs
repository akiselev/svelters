use std::rc::Rc;

use super::*;

impl Module for FwCore {
    const MODULE_NAME: &'static str = "FwCore";
    #[inline(always)]
    fn define<P>(
        m: ModuleBuilder<Self, P>,
    ) -> std::result::Result<ModuleBuilder<Self, P>, rquickjs::Error> {
        m.class::<Fw<Rc<Object>>>()?.class::<Fw<Rc<Application>>>()
    }
}

impl Module for FwWidgets {
    const MODULE_NAME: &'static str = "FwWidgets";
    #[inline(always)]
    fn define<P>(
        m: ModuleBuilder<Self, P>,
    ) -> std::result::Result<ModuleBuilder<Self, P>, rquickjs::Error> {
        m.class::<Fw<Rc<Object>>>()?
            .class::<Fw<Rc<Application>>>()?
            .class::<Fw<Rc<Widget>>>()?
            .class::<Fw<Rc<MainWindow>>>()?
            .class::<Fw<Rc<BoxLayout>>>()?
            .class::<Fw<Rc<AbstractButton>>>()?
            .class::<Fw<Rc<PushButton>>>()
    }
}
