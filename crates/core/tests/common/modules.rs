use std::rc::Rc;

use super::*;

impl Module<FW> for FwCore {
    fn define<P>(m: &mut ModuleBuilder<FW, Self, P>) -> std::result::Result<(), rquickjs::Error> {
        m.class::<Fw<Rc<Object>>>()?;
        m.class::<Fw<Rc<Application>>>()?;
        Ok(())
    }
}

impl Module<FW> for FwWidgets {
    fn define<P>(m: &mut ModuleBuilder<FW, Self, P>) -> std::result::Result<(), rquickjs::Error> {
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
