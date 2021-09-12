#[macro_export(crate)]
macro_rules! build{($cls:ident::$phase:ty[$($args:expr),*]) => {{
    let mut child = <$phase>::new($($args),*);
    let mut cls: TypeBuilder<$cls, _> = TypeBuilder::new(child);
    let res = <$cls as Type>::define(cls)?;
    res
}}}

#[macro_export(crate)]
macro_rules! build_class {
    ($cls:ident with $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$cls, _> = TypeBuilder::new(child);
        <$cls as Type>::define(cls)?
    };};
    ($cls:ident with phase $phase:expr) => {{
        let mut child = $phase;
        let mut cls: TypeBuilder<$cls, _> = TypeBuilder::new(child);
        cls.class()?;
        <$cls as Type>::define(cls)?
    };};
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

#[macro_export(crate)]
macro_rules! check_member{
    ($cls:ty, $phase:ident, $name:expr) => {
        if build_class!($cls with HasMember::<'a, $cls>::new($name)).is_set {
            return Ok(phase);
        }
        if build_class!($cls with InheritsMember::<'a, $cls>::new($name, $phase.depth - 1))
            .is_set
        {
            return Ok(phase);
        }
    }
}
