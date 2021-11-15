use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::prelude::BoxExt;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Opt, Value};

use crate::{common::*, core::enums::Enum, RegisterType};

macro def($name:ident as $ty:ty as $lit:literal) {
    pub type $name = Gtk<$ty>;

    interface!($name as $ty | ty | {});

    class_def!(
        $name as $lit | ty | {
            ty.implements::<$name>()?;
            ty.implements::<crate::Object>()?;
            ty.register_as::<$ty>()?;
        }
    );
}

def!(Accessible as gtk4::Accessible as "Accessible");
def!(Actionable as gtk4::Actionable as "Actionable");
def!(AppChooser as gtk4::AppChooser as "AppChooser");
def!(Buildable as gtk4::Buildable as "Buildable");
def!(ConstraintTarget as gtk4::ConstraintTarget as "ConstraintTarget");
def!(Native as gtk4::Native as "Native");
def!(Root as gtk4::Root as "Root");
def!(ShortcutManager as gtk4::ShortcutManager as "ShortcutManager");
def!(Orientable as gtk4::Orientable as "Orientable");
def!(CellLayout as gtk4::CellLayout as "CellLayout");
def!(CellEditable as gtk4::CellEditable as "CellEditable");
def!(Editable as gtk4::Editable as "Editable");
def!(ColorChooser as gtk4::ColorChooser as "ColorChooser");
def!(FontChooser as gtk4::FontChooser as "FontChooser");
def!(FileChooser as gtk4::FileChooser as "FileChooser");
def!(Scrollable as gtk4::Scrollable as "Scrollable");
def!(StyleProvider as gtk4::StyleProvider as "StyleProvider");
def!(TreeDragDest as gtk4::TreeDragDest as "TreeDragDest");
def!(TreeDragSource as gtk4::TreeDragSource as "TreeDragSource");
def!(TreeModel as gtk4::TreeModel as "TreeModel");
def!(TreeSortable as gtk4::TreeSortable as "TreeSortable");
def!(PrintOperationPreview as gtk4::PrintOperationPreview as "PrintOperationPreview");
def!(SelectionModel as gtk4::SelectionModel as "SelectionModel");

def!(Paintable as gdk4::Paintable as "Paintable");

def!(ListModel as gio::ListModel as "ListModel");
def!(MountOperation as gio::MountOperation as "MountOperation");
def!(Initable as gio::Initable as "Initable");
def!(AppLaunchContext as gio::AppLaunchContext as "AppLaunchContext");
def!(AsyncResult as gio::AsyncResult as "AsyncResult");

def!(InitiallyUnowned as gtk4::glib::InitiallyUnowned as "InitiallyUnowned");

pub fn initialize_interfaces<'js>(ctx: Ctx<'js>) -> rquickjs::Result<()> {
    ctx.register_type::<Gtk<gtk4::Accessible>>("Accessible")?;
    ctx.register_type::<Gtk<gtk4::Actionable>>("Actionable")?;
    ctx.register_type::<Gtk<gtk4::AppChooser>>("AppChooser")?;
    ctx.register_type::<Gtk<gtk4::Buildable>>("Buildable")?;
    ctx.register_type::<Gtk<gtk4::ConstraintTarget>>("ConstraintTarget")?;
    ctx.register_type::<Gtk<gtk4::Native>>("Native")?;
    ctx.register_type::<Gtk<gtk4::Root>>("Root")?;
    ctx.register_type::<Gtk<gtk4::ShortcutManager>>("ShortcutManager")?;
    ctx.register_type::<Gtk<gtk4::Orientable>>("Orientable")?;
    ctx.register_type::<Gtk<gtk4::CellLayout>>("CellLayout")?;
    ctx.register_type::<Gtk<gtk4::CellEditable>>("CellEditable")?;
    ctx.register_type::<Gtk<gtk4::Editable>>("Editable")?;
    ctx.register_type::<Gtk<gtk4::ColorChooser>>("ColorChooser")?;
    ctx.register_type::<Gtk<gtk4::FontChooser>>("FontChooser")?;
    ctx.register_type::<Gtk<gtk4::FileChooser>>("FileChooser")?;
    ctx.register_type::<Gtk<gtk4::Scrollable>>("Scrollable")?;
    ctx.register_type::<Gtk<gtk4::StyleProvider>>("StyleProvider")?;
    ctx.register_type::<Gtk<gtk4::TreeDragDest>>("TreeDragDest")?;
    ctx.register_type::<Gtk<gtk4::TreeDragSource>>("TreeDragSource")?;
    ctx.register_type::<Gtk<gtk4::TreeModel>>("TreeModel")?;
    ctx.register_type::<Gtk<gtk4::TreeSortable>>("TreeSortable")?;
    ctx.register_type::<Gtk<gtk4::PrintOperationPreview>>("PrintOperationPreview")?;
    ctx.register_type::<Gtk<gtk4::SelectionModel>>("SelectionModel")?;
    ctx.register_type::<Gtk<gdk4::Paintable>>("Paintable")?;
    ctx.register_type::<Gtk<gio::ListModel>>("ListModel")?;
    ctx.register_type::<Gtk<gio::MountOperation>>("MountOperation")?;
    ctx.register_type::<Gtk<gio::Initable>>("Initable")?;
    ctx.register_type::<Gtk<gio::AppLaunchContext>>("AppLaunchContext")?;
    ctx.register_type::<Gtk<gio::AsyncResult>>("AsyncResult")?;
    ctx.register_type::<Gtk<gtk4::glib::InitiallyUnowned>>("InitiallyUnowned")?;
    Ok(())
}
