use std::rc::Rc;

use super::*;

impl Module for FwCore {
    #[inline(always)]
    fn define<P>(m: &mut ModuleBuilder<Self, P>) -> std::result::Result<(), rquickjs::Error> {
        m.class::<Fw<Rc<Object>>>()?;
        m.class::<Fw<Rc<Application>>>()?;
        Ok(())
    }
}

impl Module for FwWidgets {
    #[inline(always)]
    fn define<P>(m: &mut ModuleBuilder<Self, P>) -> std::result::Result<(), rquickjs::Error> {
        m.class::<Fw<Rc<Object>>>()?;
        m.class::<Fw<Rc<Application>>>()?;
        m.class::<Fw<Rc<Widget>>>()?;
        m.class::<Fw<Rc<MainWindow>>>()?;
        m.class::<Fw<Rc<BoxLayout>>>()?;
        m.class::<Fw<Rc<AbstractButton>>>()?;
        m.class::<Fw<Rc<PushButton>>>()?;
        Ok(())
    }
}
