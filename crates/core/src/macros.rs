#[macro_export(crate)]
macro_rules! build_class {
    ($cls:ident with $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$cls, _> = TypeBuilder::new(&mut child);
        cls.class()?;
        <$cls as Type>::define(&mut cls)?;
        child
    }};
    ($cls:ident with phase $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$cls, _> = TypeBuilder::new(&mut child);
        cls.class()?;
        <$cls as Type>::define(&mut cls)?;
        child
    }};
}
