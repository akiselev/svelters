use frunk::{hlist, Hlist};

use crate::inner::*;

pub struct Builder<T, D> {
    name: Cow<'static, str>,
    _type: PhantomData<T>,
    list: D,
}

pub const fn class<V>() -> Builder<V, HNil> {
    Builder {
        name: Cow::Borrowed(std::any::type_name::<V>()),
        _type: PhantomData,
        list: HNil,
    }
}

impl<T, D> Builder<T, D> {
    pub fn with_constructor<U, M: Func<U, Output = T>>(
        self,
        other: M,
    ) -> Builder<T, HCons<Constructor<T, U, M>, D>> {
        Builder {
            name: self.name,
            _type: self._type,
            list: HCons {
                head: Constructor::new(other),
                tail: self.list,
            },
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
