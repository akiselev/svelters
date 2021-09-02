use std::future::Future;

use rquickjs::{Loader, Resolver};

use crate::{build_class, inner::*};

/// The framework that Svelters exposes to Javascript and Svelte
pub trait Framework: Sized + 'static {
    type Loader: Loader;
    type Resolver: Resolver;
    type AppTask: Future<Output = anyhow::Result<()>>;

    fn name() -> &'static str;
    fn get_loader() -> (Self::Resolver, Self::Loader);
    fn init_app(ctx: Context) -> Result<Self::AppTask>;

    fn module<MOD>(module: MOD) -> JsModuleLoader<MOD>
    where
        MOD: Module,
    {
        JsModuleLoader::new(module)
    }
}

/// Cast Self to type &T
pub trait Cast<T> {
    type Output<'a>: AsRef<T>;

    fn cast<'a>(&'a self) -> Self::Output<'a>;
}

pub type ReprOf<CLS> = <CLS as Repr>::Type;
pub type CastedAs<'a, CLS, T> = <<CLS as Repr>::Type as Cast<T>>::Output<'a>;

/// The representation Svelters will use when storing a type
///
/// Since `svelters` bridges Svelte with external GUI libraries, users usually
/// can't implement our traits for the types directly beacuse of Rust's orphan
/// rule. This trait supplies a custom representation (wrapper) of external
/// types so users can implement our traits
pub trait Repr {
    type Type;
}

/// Converts a type into the internal representation used by Svelters
///
/// This is a separate trait from [Repr] because the framework Svelters wraps
/// might wrap its types in Box<T>, Rc<T>, or custom framework types.
///
/// For example, the Ritual QT libraries return QBox<T>, QShared<T>, etc to
/// represent ownership
pub trait IntoRepr<CLS>
where
    CLS: Repr,
{
    fn into_repr(self) -> <CLS as Repr>::Type;
}

/// Defines the contents of a Javascript module using a [ModuleBuilder]
pub trait Module: Sized {
    #[inline(always)]
    fn define<P>(module: &mut ModuleBuilder<Self, P>) -> Result<()>;
}

/// Defines a framework type that is exposed to Javascript using a [TypeBuilder]
pub trait Type: 'static + Sized + Repr {
    const CLASS_NAME: &'static str;

    unsafe fn metadata() -> &'static mut Metadata;

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()>;

    fn name() -> &'static str {
        Self::CLASS_NAME
    }
}

/// Implementation for a phase
///
/// This trait is specialized for all types P and T:
///
/// ```rust
/// impl<T, P> Implements<P> for T {
///     default fn build(self, _phase: &mut P) -> Result<()> {
///         Ok(())
///     }
/// }
/// ```
///
/// When building up Javascript types, Svelters calls [Type::define] repeatedly
/// with different phases like [InitProto], [InitStatic], [InitSuperProto] and
/// so on. The rust compiler should optimize away the default impl and inline
/// any concrete implementations (if `#[inline]` attribute is applied or if LLVM
/// decides it makes sense).
///
/// Types [Constructor], [Method], [Superclass], and so on each implement the
/// relevant phases to build up the type. The [Method] builder, for example,
/// implement the [InitProto] phase which provides the [rquickjs::Object] that
/// represents the Javascript class prototype. In the [InitProto] phase the
/// [Method] calls [rquickjs::Object::set] with a wrapper closure that converts
/// calls the user provided method closure.
///
/// Since this trait is specialized for all type pairs, the calls in
/// [Type::define] act like a bytecode that changes its functionality depending
/// on which phase it's called in. Since the reference to the phase is mutable,
/// implementations can also pass information back to the caller to implement
/// features like reflection and debuging.
pub trait Implements<P>: Sized {
    #[inline(always)]
    fn build(self, phase: &mut P) -> Result<()> {
        Ok(())
    }
}

// impl<T, P> Implements<P> for T {
//     #[inline(always)]
//     default fn build(self, _phase: &mut P) -> Result<()> {
//         Ok(())
//     }
// }

#[inline(always)]
pub fn test_inline_has_member<'a, CLS>(name: &'a str) -> anyhow::Result<bool>
where
    CLS: Type,
{
    Ok(build_class!(CLS with HasMember::<'a, CLS>::new(name)).is_set)
}

#[inline(always)]
pub fn test_inline_inherits_member<'a, CLS>(name: &'a str, max_depth: isize) -> anyhow::Result<bool>
where
    CLS: Type,
{
    Ok(build_class!(CLS with InheritsMember::<'a, CLS>::new(name, max_depth)).is_set)
}
