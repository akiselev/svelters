use std::{backtrace::Backtrace, future::Future};

use rquickjs::{Loader, Resolver};

use crate::inner::*;

/// Cast Self to type &T
pub trait Upcast<T> {
    fn upcast(&self) -> T;
}

pub trait UpcastRef<T> {
    fn upcast_ref(&self) -> &T;
}

/// Defines the contents of a Javascript module using a [ModuleBuilder]
pub trait Module: Sized {
    const MODULE_NAME: &'static str;

    fn define<P>(module: ModuleBuilder<Self, P>) -> Result<ModuleBuilder<Self, P>>;

    #[inline(always)]
    fn loader(self) -> SveltersModule<Self> {
        SveltersModule::new(self)
    }
}

/// Defines a framework type that is exposed to Javascript using a [TypeBuilder]
pub trait Type: 'static + Sized {
    const CLASS_NAME: &'static str;

    unsafe fn metadata() -> &'static mut Metadata;

    fn define<P>(cls: TypeBuilder<Self, P>) -> Result<()>;

    #[inline(always)]
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
///     default fn build_phase(self, _phase: &mut P) -> Result<()> {
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
    fn build_phase(self, _phase: P) -> Result<P>;

    // fn build_phase_logged(self, phase: &mut P) -> Result<()> {
    //     tracing::info!("Calling phase <{}> for <{}>", std::any::type_name::<P>(), std::any::type_name::<Self>());
    //     self.build_phase(phase)
    // }
}

impl<T, P> Implements<P> for T {
    #[inline(always)]
    default fn build_phase(self, phase: P) -> Result<P> {
        Ok(phase)
    }
}

/// Core trait for slot implementations (provides DOM APIs)
pub trait INativeSlot: 'static + Clone {
    fn append_child<'js>(&self, this: Node<'js>, node: Node<'js>) -> Result<()>;
    fn remove_child<'js>(&self, this: Node<'js>, node: Node<'js>) -> Result<()>;
    fn insert_before<'js>(
        &self,
        this: Node<'js>,
        node: Node<'js>,
        anchor: Option<Node<'js>>,
    ) -> Result<()>;
}

pub trait INode {
    fn next<'js>(this: Node<'js>) -> Option<Node<'js>>;
    fn prev<'js>(this: Node<'js>) -> Option<Node<'js>>;
    fn parent<'js>(this: Node<'js>) -> Option<Node<'js>>;
    fn children<'js>(this: Node<'js>) -> Option<Vec<Node<'js>>>;

    fn node_name<'js>(&self, ctx: Ctx<'js>) -> String;
    fn get_root_node<'js>(&self, ctx: Ctx<'js>) -> Option<Object<'js>>;
}

pub trait IElement<CLS>: 'static {
    fn constructor<'js>(&self, ctx: Ctx<'js>) -> Result<CLS>;
}
