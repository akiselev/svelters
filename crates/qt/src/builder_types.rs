use crate::inner::*;

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
pub struct Constructor<T, FW, A, FN>
where
    FW: Framework,
    FN: Func<A>,
    <FN as Func<A>>::Output: Container<FW, T>,
{
    fw: FW,
    method: FN,
    args: PhantomData<A>,
    _type: PhantomData<T>,
}

impl<T, A, FW, FN> Constructor<T, FW, A, FN>
where
    FW: Framework,
    FN: Func<A>,
    <FN as Func<A>>::Output: Container<FW, T>,
{
    pub(in crate) const fn new(fw: FW, method: FN) -> Self {
        Self {
            fw,
            method,
            args: PhantomData,
            _type: PhantomData,
        }
    }
}

impl<T, A, FW, FN> Member<T> for Constructor<T, FW, A, FN>
where
    FW: Framework,
    FN: Func<A>,
    <FN as Func<A>>::Output: Container<FW, T>,
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
