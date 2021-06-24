use crate::inner::*;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Core<T: Framework>(T);

impl<FW: Framework> Core<FW> {
    pub const fn new(fw: FW) -> Self {
        Self(fw)
    }

    pub const fn init<T, A, FN>(self, method: FN) -> Constructor<T, FW, A, FN>
    where
        FN: Func<A>,
        <FN as Func<A>>::Output: Container<FW, T>,
    {
        Constructor::new(self.0, method)
    }

    pub const fn cls<V, D: Member<V>>(self, _type: D) -> Class<V, D, FW> {
        Class {
            name: Cow::Borrowed(std::any::type_name::<V>()),
            _type: PhantomData,
            list: _type,
            fw: self.0,
        }
    }
}

pub enum TypeKind {
    Primitive,
    Struct,
    ForeignStruct,
    Class,
}

pub trait TypeDef {
    fn type_name(&self) -> Cow<'static, str>;
}
