use crate::inner::*;

// default impl<FW: Framework + FrameworkClass<NAME>, const CLS: &'static str> Cast<FW, NAME> for V {
//     type Output<'a, U: 'a>
//     where
//         V: 'a,
//         T: 'a,
//     = DefaultCast<'a, U>;

//     fn cast<'a>(&'a self) -> Option<Self::Output<'a, T>>
//     where
//         T: 'a,
//     {
//         None
//     }
// }

// pub struct DefaultCast<'a, T: 'a>(pub(in crate) PhantomData<&'a T>);
// impl<'a, T: 'a> AsRef<T> for DefaultCast<'a, T> {
//     fn as_ref(&self) -> &T {
//         panic!("This shouldn't have happened")
//     }
// }
