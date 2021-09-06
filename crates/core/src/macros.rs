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

#[macro_export]
macro_rules! exception {
    ($($tokens:tt)*) => {
        {
            let backtrace = std::backtrace::Backtrace::capture().to_string();
            rquickjs::Error::Exception {
                message: format!(
                    $($tokens)*
                ),
                file: file!().to_string(),
                line: line!().try_into().unwrap(),
                stack: backtrace.clone(),
                rust_stack: Some(backtrace),
            }
        }
    };
}
