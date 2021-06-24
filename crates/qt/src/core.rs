use crate::inner::*;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Core<T: Framework>(T);

impl<T: Framework> Core<T> {
    pub const fn new(fw: T) -> Self {
        Self(fw)
    }

    pub const fn class<V, D>(self, _type: D) -> Class<V, D, T> {
        Class {
            name: Cow::Borrowed(std::any::type_name::<V>()),
            _type: PhantomData,
            list: _type,
            fw: self.0,
        }
    }

    pub const fn as_class<V>(self, val: &V) -> Class<V, HNil, T> {
        Class {
            name: Cow::Borrowed(std::any::type_name::<V>()),
            _type: PhantomData,
            list: HNil,
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
