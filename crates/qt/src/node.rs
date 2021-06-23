use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::Add;

use frunk::{HCons, HNil};
use rquickjs::{Ctx, Error, FromJs, IntoJs, Result, Value};

use crate::TypeDef;

pub struct NodeType<T> {
    name: Cow<'static, str>,
    list: T,
}

impl<T> NodeType<T> {
    pub fn new<N: Into<Cow<'static, str>>>(name: N) -> NodeType<HNil> {
        NodeType {
            name: name.into(),
            list: HNil,
        }
    }

    pub fn with<U>(NodeType { name, list }: Self, other: U) -> NodeType<<T as Add<U>>::Output>
    where
        T: Add<U>,
    {
        NodeType {
            name,
            list: list + other,
        }
    }

    // pub fn build(&self) -> &dyn TypeDef
    // where
    //     T: BuildClass,
    // {
    //     self.list.build(&self.name)
    // }
}
