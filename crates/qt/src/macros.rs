use crate::*;

// #[macro_export]
// macro_rules! type_member {
//     ($name:ident { $($member:ident: $tokens:tt),* }) => {
//         // type_member!($name { $tokens })
//     };
// }

// #[macro_export]
// macro_rules! type_class {
//     ($name:ty { $($member:ident: $expr:expr),* }) => {
//         type_member!($($member: $expr),*)
//     }
// }

// #[macro_export]
// macro_rules! typed {
//     (class $name:ty { $($member:ident: $expr:expr),* }) => {
//         type_class!(::svelters_qt::class(stringify!($name)) => $name { $($member: $expr),* });
//     };
// }
