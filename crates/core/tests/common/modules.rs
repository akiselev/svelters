use super::*;

impl Module<Fw> for FwCore {
    fn define<P>(m: &mut ModuleBuilder<Fw, Self, P>) -> std::result::Result<(), rquickjs::Error> {
        m.class::<Object>()?;
        m.class::<Application>()?;
        Ok(())
    }
}

impl Module<Fw> for FwWidgets {
    fn define<P>(m: &mut ModuleBuilder<Fw, Self, P>) -> std::result::Result<(), rquickjs::Error> {
        m.class::<Object>()?;
        m.class::<Application>()?;
        m.class::<Widget>()?;
        m.class::<MainWindow>()?;
        m.class::<BoxLayout>()?;
        m.class::<AbstractButton>()?;
        m.class::<PushButton>()?;
        Ok(())
    }
}
