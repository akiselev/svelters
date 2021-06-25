use crate::inner::*;

impl<FW: Framework, const NAME: &'static str> JsClass<FW, NAME>
where
    FW: FrameworkClass<NAME>,
{
    pub const fn constructor<A, FN>(self, method: FN) -> Constructor<NAME, FW, A, FN>
    where
        FN: Func<A>,
        <FN as Func<A>>::Output: Cast<<FW as FrameworkClass<NAME>>::Wrapper>,
    {
        Constructor::new(method)
    }
}

pub struct Superclass<T, R, F: Fn(&T) -> Option<&R>> {
    func: F,
    source: PhantomData<T>,
    target: PhantomData<R>,
}

impl<T, R, F: Fn(&T) -> Option<&R>> Superclass<T, R, F> {
    pub const fn new(func: F) -> Self {
        Self {
            func,
            source: PhantomData,
            target: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Constructor<const T: &'static str, FW, A, FN>
where
    FW: Framework + FrameworkClass<T>,
    FN: Func<A>,
    <FN as Func<A>>::Output: Convert<<FW as FrameworkClass<T>::Wrapper>>,
{
    fw: PhantomData<FW>,
    method: FN,
    args: PhantomData<A>,
}

impl<const T: &'static str, A, FW, FN> Constructor<T, FW, A, FN>
where
    FW: Framework + FrameworkClass<T>,
    FN: Func<A>,
    <FN as Func<A>>::Output: Convert<<FW as FrameworkClass<T>::Wrapper>>,
{
    pub(in crate) const fn new(method: FN) -> Self {
        Self {
            method,
            fw: PhantomData,
            args: PhantomData,
        }
    }
}

impl<const T: &'static str, A, FW, FN> JsClassDef for Constructor<T, FW, A, FN>
where
    FW: Framework + FrameworkClass<T>,
    FN: Func<A>,
    <FN as Func<A>>::Output: Convert<<FW as FrameworkClass<T>::Wrapper>>,
{
}

impl<const T: &'static str, A, FW, FN> Member<T> for Constructor<T, FW, A, FN>
where
    FW: Framework + FrameworkClass<T>,
    FN: Func<A>,
    <FN as Func<A>>::Output: Convert<<FW as FrameworkClass<T>::Wrapper>>,
{
}

pub struct Getter<T, R, F: Fn(&T) -> R> {
    name: Cow<'static, str>,
    func: F,
    _class: PhantomData<T>,
    _type: PhantomData<R>,
}

impl<T, R, F: Fn(&T) -> R> Getter<T, R, F> {
    pub const fn new(name: Cow<'static, str>, func: F) -> Self {
        Self {
            name,
            func,
            _class: PhantomData,
            _type: PhantomData,
        }
    }
}

pub struct Setter<T, R, F: Fn(&T) -> R> {
    name: Cow<'static, str>,
    func: F,
    _class: PhantomData<T>,
    _type: PhantomData<R>,
}

impl<T, R, F: Fn(&T) -> R> Setter<T, R, F> {
    pub const fn new(name: Cow<'static, str>, func: F) -> Self {
        Self {
            name,
            func,
            _class: PhantomData,
            _type: PhantomData,
        }
    }
}
