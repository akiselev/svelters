#[macro_export(crate)]
macro_rules! build_class {
    ($fw:ident::$cls:ident with $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$fw, $cls, _> = TypeBuilder::new(&mut child);
        cls.class()?;
        <$cls as Type<$fw>>::define(&mut cls)?;
        child
    }};
    ($fw:ident::$cls:ident with phase $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$fw, $cls, _> = TypeBuilder::new(&mut child);
        cls.class()?;
        <$cls as Type<$fw>>::define(&mut cls)?;
        child
    }};
}
