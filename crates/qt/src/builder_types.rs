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

pub struct Constructor<T, A, F: Func<A, Output = T>> {
    method: F,
    _type: PhantomData<T>,
    args: PhantomData<A>,
}

impl<T, A, F: Func<A, Output = T>> Constructor<T, A, F> {
    pub const fn new(method: F) -> Self {
        Self {
            method,
            _type: PhantomData,
            args: PhantomData,
        }
    }
}

pub struct Function<A, F: Func<A>> {
    method: F,
    args: PhantomData<A>,
}

impl<A, F: Func<A>> Function<A, F> {
    pub const fn new(method: F) -> Self {
        Self {
            method,
            args: PhantomData,
        }
    }
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
