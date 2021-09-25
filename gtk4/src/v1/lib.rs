#![allow(warnings)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(in_band_lifetimes)]
#![feature(backtrace)]
#![feature(decl_macro)]
#![feature(proc_macro_hygiene)]
#![feature(let_chains)]
#![feature(trait_alias)]
#![recursion_limit = "1024"]

use std::{borrow::Cow, future::Future, time::Duration};

use glib::MainContext;
use rquickjs::{
    BuiltinResolver, Context, Ctx, Error, Executor, ExecutorSpawner, Loader, ModuleLoader,
    Resolver, Result, Value,
};
use static_assertions::assert_impl_all;
use svelters_core::prelude::*;

pub use crate::wrapper::Gtk;

mod jsenum;
mod macros;
mod types;
mod wrapper;

pub(in crate) mod inner {
    pub(in crate) use std::{
        borrow::Cow,
        cell::Cell,
        convert::{TryFrom, TryInto},
        marker::PhantomData,
        ops::{Add, Deref, DerefMut},
    };

    pub(in crate) use frunk::{hlist, hlist::Selector, HCons, HNil};
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use rquickjs::{
        ClassDef, ClassId, Context, Ctx, Error, FromJs, IntoJs, ModuleDef, Object, RefsMarker,
        Result, Value,
    };

    pub(in crate) use svelters_core::{exception, prelude::*};

    pub(in crate) use crate::jsenum::*;
    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::wrapper::Gtk;
}
mod module {
    use super::Gtk;
    use glib::object::Object as GObject;
    use gtk4::{gsk::Transform, *};

    crate::macros::module!(
        /***** Core *****/
        Gtk: GObject,
        Widget,
        Application,
        /***** Window *****/
        Window,
        ApplicationWindow,
        /***** Layout *****/
        Transform,
        LayoutChild,
        ConstraintLayoutChild,
        FixedLayoutChild,
        GridLayoutChild,
        OverlayLayoutChild,
        LayoutManager,
        /***** Accessibility *****/
        Accessible,
        /***** Buttons *****/
        Button,
        /***** WIP *****/
        ActionBar,
        Actionable,
        AppChooserButton,
        AppChooserWidget,
        AppChooser,
        AspectFrame,
        Box,
        Calendar,
        CellEditable,
        CellView,
        CenterBox,
        CheckButton,
        ColorButton,
        ColorChooserWidget,
        ColumnView,
        ComboBox,
        DragIcon,
        DrawingArea,
        DropDown,
        EditableLabel,
        Editable,
        Entry,
        Expander,
        FileChooserWidget,
        Fixed,
        FlowBoxChild,
        FlowBox,
        FontButton,
        FontChooserWidget,
        Frame,
        GLArea,
        Grid,
        HeaderBar,
        IconView,
        Image,
        InfoBar,
        Label,
        LevelBar,
        ListBase,
        // ListBoxRow,
        // ListBox,
        // MediaControls,
        // MenuButton,
        // Native,
        // Notebook,
        // Overlay,
        // Paned,
        // PasswordEntry,
        // Picture,
        // PopoverMenuBar,
        // Popover,
        // ProgressBar,
        // Range,
        // Revealer,
        // Root,
        // ScaleButton,
        // Scrollbar,
        // ScrolledWindow,
        // SearchBar,
        // SearchEntry,
        // Separator,
        // ShortcutLabel,
        // ShortcutsShortcut,
        // SpinButton,
        // Spinner,
        // StackSidebar,
        // StackSwitcher,
        // Stack,
        // Statusbar,
        // Switch,
        // TextView,
        // Text,
        // TreeExpander,
        // TreeView,
        // Video,
        // Viewport,
        // WindowControls,
        // WindowHandle,
    );
}

pub struct GtkExecutor;

impl ExecutorSpawner for GtkExecutor {
    type JoinHandle = ();

    fn spawn_executor(self, task: Executor) -> Self::JoinHandle {
        let ctx = MainContext::default();
        ctx.spawn_local(task)
    }
}

pub fn spawn_local(fut: impl 'static + Future<Output = ()>) {
    let ctx = MainContext::default();
    ctx.spawn_local(fut)
}
