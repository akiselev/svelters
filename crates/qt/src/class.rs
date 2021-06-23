use crate::inner::*;

pub struct Builder<T, D> {
    name: Cow<'static, str>,
    _type: PhantomData<T>,
    list: D,
}

pub fn class<V>(name: &'static str) -> Builder<V, HNil> {
    Builder {
        name: name.into(),
        _type: PhantomData,
        list: HNil,
    }
}

impl<T, D> Builder<T, D> {
    pub fn with_constructor<U, M: Func<U, Output = T>>(
        self,
        other: M,
    ) -> Builder<T, <D as Add<Constructor<T, U, M>>>::Output>
    where
        D: Add<Constructor<T, U, M>>,
    {
        Builder {
            name: self.name,
            _type: self._type,
            list: self.list + Constructor::new(other),
        }
    }

    pub fn with<U>(self, other: U) -> Builder<T, <D as Add<U>>::Output>
    where
        D: Add<U>,
    {
        Builder {
            name: self.name,
            _type: self._type,
            list: self.list + other,
        }
    }

    // pub fn build(&self) -> impl TypeDef
    // where
    //     D: BuildClass,
    // {
    //     self.list.build(&self.name)
    // }
}
