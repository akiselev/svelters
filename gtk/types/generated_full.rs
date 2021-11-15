use crate::RegisterType;

pub type AppLaunchContext = crate::newtype::Gtk<gdk4::AppLaunchContext>;

crate::common::interface!(AppLaunchContext as gdk4::AppLaunchContext | _ty | {});

crate::common::class_def!(
    AppLaunchContext as "AppLaunchContext" |ty| {
        crate::core::from_gtype::define::<gdk4::AppLaunchContext, _>(&ty)?;
        ty.implements::<AppLaunchContext>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::AppLaunchContext>>()?;
        ty.register_as::<gdk4::AppLaunchContext>()?;
    }
);

pub type CairoContext = crate::newtype::Gtk<gdk4::CairoContext>;

crate::common::interface!(CairoContext as gdk4::CairoContext | _ty | {});

crate::common::class_def!(
    CairoContext as "CairoContext" |ty| {
        crate::core::from_gtype::define::<gdk4::CairoContext, _>(&ty)?;
        ty.implements::<CairoContext>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::DrawContext>>()?;
        ty.register_as::<gdk4::CairoContext>()?;
    }
);

pub type Clipboard = crate::newtype::Gtk<gdk4::Clipboard>;

crate::common::interface!(Clipboard as gdk4::Clipboard | _ty | {});

crate::common::class_def!(
    Clipboard as "Clipboard" |ty| {
        crate::core::from_gtype::define::<gdk4::Clipboard, _>(&ty)?;
        ty.implements::<Clipboard>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Clipboard>()?;
    }
);

pub type ContentDeserializer = crate::newtype::Gtk<gdk4::ContentDeserializer>;

crate::common::interface!(ContentDeserializer as gdk4::ContentDeserializer | _ty | {});

crate::common::class_def!(
    ContentDeserializer as "ContentDeserializer" |ty| {
        crate::core::from_gtype::define::<gdk4::ContentDeserializer, _>(&ty)?;
        ty.implements::<ContentDeserializer>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::AsyncResult>>()?;
        ty.register_as::<gdk4::ContentDeserializer>()?;
    }
);

pub type ContentProvider = crate::newtype::Gtk<gdk4::ContentProvider>;

crate::common::interface!(ContentProvider as gdk4::ContentProvider | _ty | {});

crate::common::class_def!(
    ContentProvider as "ContentProvider" |ty| {
        crate::core::from_gtype::define::<gdk4::ContentProvider, _>(&ty)?;
        ty.implements::<ContentProvider>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::ContentProvider>()?;
    }
);

pub type ContentSerializer = crate::newtype::Gtk<gdk4::ContentSerializer>;

crate::common::interface!(ContentSerializer as gdk4::ContentSerializer | _ty | {});

crate::common::class_def!(
    ContentSerializer as "ContentSerializer" |ty| {
        crate::core::from_gtype::define::<gdk4::ContentSerializer, _>(&ty)?;
        ty.implements::<ContentSerializer>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::AsyncResult>>()?;
        ty.register_as::<gdk4::ContentSerializer>()?;
    }
);

pub type Cursor = crate::newtype::Gtk<gdk4::Cursor>;

crate::common::interface!(Cursor as gdk4::Cursor | _ty | {});

crate::common::class_def!(
    Cursor as "Cursor" |ty| {
        crate::core::from_gtype::define::<gdk4::Cursor, _>(&ty)?;
        ty.implements::<Cursor>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Cursor>()?;
    }
);

pub type Device = crate::newtype::Gtk<gdk4::Device>;

crate::common::interface!(Device as gdk4::Device | _ty | {});

crate::common::class_def!(
    Device as "Device" |ty| {
        crate::core::from_gtype::define::<gdk4::Device, _>(&ty)?;
        ty.implements::<Device>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Device>()?;
    }
);

pub type DeviceTool = crate::newtype::Gtk<gdk4::DeviceTool>;

crate::common::interface!(DeviceTool as gdk4::DeviceTool | _ty | {});

crate::common::class_def!(
    DeviceTool as "DeviceTool" |ty| {
        crate::core::from_gtype::define::<gdk4::DeviceTool, _>(&ty)?;
        ty.implements::<DeviceTool>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::DeviceTool>()?;
    }
);

pub type Display = crate::newtype::Gtk<gdk4::Display>;

crate::common::interface!(Display as gdk4::Display | _ty | {});

crate::common::class_def!(
    Display as "Display" |ty| {
        crate::core::from_gtype::define::<gdk4::Display, _>(&ty)?;
        ty.implements::<Display>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Display>()?;
    }
);

pub type DisplayManager = crate::newtype::Gtk<gdk4::DisplayManager>;

crate::common::interface!(DisplayManager as gdk4::DisplayManager | _ty | {});

crate::common::class_def!(
    DisplayManager as "DisplayManager" |ty| {
        crate::core::from_gtype::define::<gdk4::DisplayManager, _>(&ty)?;
        ty.implements::<DisplayManager>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::DisplayManager>()?;
    }
);

pub type Drag = crate::newtype::Gtk<gdk4::Drag>;

crate::common::interface!(Drag as gdk4::Drag | _ty | {});

crate::common::class_def!(
    Drag as "Drag" |ty| {
        crate::core::from_gtype::define::<gdk4::Drag, _>(&ty)?;
        ty.implements::<Drag>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Drag>()?;
    }
);

pub type DrawContext = crate::newtype::Gtk<gdk4::DrawContext>;

crate::common::interface!(DrawContext as gdk4::DrawContext | _ty | {});

crate::common::class_def!(
    DrawContext as "DrawContext" |ty| {
        crate::core::from_gtype::define::<gdk4::DrawContext, _>(&ty)?;
        ty.implements::<DrawContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::DrawContext>()?;
    }
);

pub type Drop = crate::newtype::Gtk<gdk4::Drop>;

crate::common::interface!(Drop as gdk4::Drop | _ty | {});

crate::common::class_def!(
    Drop as "Drop" |ty| {
        crate::core::from_gtype::define::<gdk4::Drop, _>(&ty)?;
        ty.implements::<Drop>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Drop>()?;
    }
);

pub type FrameClock = crate::newtype::Gtk<gdk4::FrameClock>;

crate::common::interface!(FrameClock as gdk4::FrameClock | _ty | {});

crate::common::class_def!(
    FrameClock as "FrameClock" |ty| {
        crate::core::from_gtype::define::<gdk4::FrameClock, _>(&ty)?;
        ty.implements::<FrameClock>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::FrameClock>()?;
    }
);

pub type GLContext = crate::newtype::Gtk<gdk4::GLContext>;

crate::common::interface!(GLContext as gdk4::GLContext | _ty | {});

crate::common::class_def!(
    GLContext as "GLContext" |ty| {
        crate::core::from_gtype::define::<gdk4::GLContext, _>(&ty)?;
        ty.implements::<GLContext>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::DrawContext>>()?;
        ty.register_as::<gdk4::GLContext>()?;
    }
);

pub type GLTexture = crate::newtype::Gtk<gdk4::GLTexture>;

crate::common::interface!(GLTexture as gdk4::GLTexture | _ty | {});

crate::common::class_def!(
    GLTexture as "GLTexture" |ty| {
        crate::core::from_gtype::define::<gdk4::GLTexture, _>(&ty)?;
        ty.implements::<GLTexture>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Texture>>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gdk4::GLTexture>()?;
    }
);

pub type MemoryTexture = crate::newtype::Gtk<gdk4::MemoryTexture>;

crate::common::interface!(MemoryTexture as gdk4::MemoryTexture | _ty | {});

crate::common::class_def!(
    MemoryTexture as "MemoryTexture" |ty| {
        crate::core::from_gtype::define::<gdk4::MemoryTexture, _>(&ty)?;
        ty.implements::<MemoryTexture>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Texture>>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gdk4::MemoryTexture>()?;
    }
);

pub type Monitor = crate::newtype::Gtk<gdk4::Monitor>;

crate::common::interface!(Monitor as gdk4::Monitor | _ty | {});

crate::common::class_def!(
    Monitor as "Monitor" |ty| {
        crate::core::from_gtype::define::<gdk4::Monitor, _>(&ty)?;
        ty.implements::<Monitor>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Monitor>()?;
    }
);

pub type Seat = crate::newtype::Gtk<gdk4::Seat>;

crate::common::interface!(Seat as gdk4::Seat | _ty | {});

crate::common::class_def!(
    Seat as "Seat" |ty| {
        crate::core::from_gtype::define::<gdk4::Seat, _>(&ty)?;
        ty.implements::<Seat>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Seat>()?;
    }
);

pub type Surface = crate::newtype::Gtk<gdk4::Surface>;

crate::common::interface!(Surface as gdk4::Surface | _ty | {});

crate::common::class_def!(
    Surface as "Surface" |ty| {
        crate::core::from_gtype::define::<gdk4::Surface, _>(&ty)?;
        ty.implements::<Surface>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gdk4::Surface>()?;
    }
);

pub type Texture = crate::newtype::Gtk<gdk4::Texture>;

crate::common::interface!(Texture as gdk4::Texture | _ty | {});

crate::common::class_def!(
    Texture as "Texture" |ty| {
        crate::core::from_gtype::define::<gdk4::Texture, _>(&ty)?;
        ty.implements::<Texture>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gdk4::Texture>()?;
    }
);

pub type VulkanContext = crate::newtype::Gtk<gdk4::VulkanContext>;

crate::common::interface!(VulkanContext as gdk4::VulkanContext | _ty | {});

crate::common::class_def!(
    VulkanContext as "VulkanContext" |ty| {
        crate::core::from_gtype::define::<gdk4::VulkanContext, _>(&ty)?;
        ty.implements::<VulkanContext>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::DrawContext>>()?;
        ty.implements::<crate::newtype::Gtk<gio::Initable>>()?;
        ty.register_as::<gdk4::VulkanContext>()?;
    }
);

pub type ATContext = crate::newtype::Gtk<gtk4::ATContext>;

crate::common::interface!(ATContext as gtk4::ATContext | _ty | {});

crate::common::class_def!(
    ATContext as "ATContext" |ty| {
        crate::core::from_gtype::define::<gtk4::ATContext, _>(&ty)?;
        ty.implements::<ATContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ATContext>()?;
    }
);

pub type AboutDialog = crate::newtype::Gtk<gtk4::AboutDialog>;

crate::common::interface!(AboutDialog as gtk4::AboutDialog | _ty | {});

crate::common::class_def!(
    AboutDialog as "AboutDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::AboutDialog, _>(&ty)?;
        ty.implements::<AboutDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::AboutDialog>()?;
    }
);

pub type ActionBar = crate::newtype::Gtk<gtk4::ActionBar>;

crate::common::interface!(ActionBar as gtk4::ActionBar | _ty | {});

crate::common::class_def!(
    ActionBar as "ActionBar" |ty| {
        crate::core::from_gtype::define::<gtk4::ActionBar, _>(&ty)?;
        ty.implements::<ActionBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ActionBar>()?;
    }
);

pub type ActivateAction = crate::newtype::Gtk<gtk4::ActivateAction>;

crate::common::interface!(ActivateAction as gtk4::ActivateAction | _ty | {});

crate::common::class_def!(
    ActivateAction as "ActivateAction" |ty| {
        crate::core::from_gtype::define::<gtk4::ActivateAction, _>(&ty)?;
        ty.implements::<ActivateAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::ActivateAction>()?;
    }
);

pub type Adjustment = crate::newtype::Gtk<gtk4::Adjustment>;

crate::common::interface!(Adjustment as gtk4::Adjustment | _ty | {});

crate::common::class_def!(
    Adjustment as "Adjustment" |ty| {
        crate::core::from_gtype::define::<gtk4::Adjustment, _>(&ty)?;
        ty.implements::<Adjustment>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::Adjustment>()?;
    }
);

pub type AlternativeTrigger = crate::newtype::Gtk<gtk4::AlternativeTrigger>;

crate::common::interface!(AlternativeTrigger as gtk4::AlternativeTrigger | _ty | {});

crate::common::class_def!(
    AlternativeTrigger as "AlternativeTrigger" |ty| {
        crate::core::from_gtype::define::<gtk4::AlternativeTrigger, _>(&ty)?;
        ty.implements::<AlternativeTrigger>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutTrigger>>()?;
        ty.register_as::<gtk4::AlternativeTrigger>()?;
    }
);

pub type AnyFilter = crate::newtype::Gtk<gtk4::AnyFilter>;

crate::common::interface!(AnyFilter as gtk4::AnyFilter | _ty | {});

crate::common::class_def!(
    AnyFilter as "AnyFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::AnyFilter, _>(&ty)?;
        ty.implements::<AnyFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::MultiFilter>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::AnyFilter>()?;
    }
);

pub type AppChooserButton = crate::newtype::Gtk<gtk4::AppChooserButton>;

crate::common::interface!(AppChooserButton as gtk4::AppChooserButton | _ty | {});

crate::common::class_def!(
    AppChooserButton as "AppChooserButton" |ty| {
        crate::core::from_gtype::define::<gtk4::AppChooserButton, _>(&ty)?;
        ty.implements::<AppChooserButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::AppChooser>>()?;
        ty.register_as::<gtk4::AppChooserButton>()?;
    }
);

pub type AppChooserDialog = crate::newtype::Gtk<gtk4::AppChooserDialog>;

crate::common::interface!(AppChooserDialog as gtk4::AppChooserDialog | _ty | {});

crate::common::class_def!(
    AppChooserDialog as "AppChooserDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::AppChooserDialog, _>(&ty)?;
        ty.implements::<AppChooserDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::AppChooser>>()?;
        ty.register_as::<gtk4::AppChooserDialog>()?;
    }
);

pub type AppChooserWidget = crate::newtype::Gtk<gtk4::AppChooserWidget>;

crate::common::interface!(AppChooserWidget as gtk4::AppChooserWidget | _ty | {});

crate::common::class_def!(
    AppChooserWidget as "AppChooserWidget" |ty| {
        crate::core::from_gtype::define::<gtk4::AppChooserWidget, _>(&ty)?;
        ty.implements::<AppChooserWidget>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::AppChooser>>()?;
        ty.register_as::<gtk4::AppChooserWidget>()?;
    }
);

pub type AspectFrame = crate::newtype::Gtk<gtk4::AspectFrame>;

crate::common::interface!(AspectFrame as gtk4::AspectFrame | _ty | {});

crate::common::class_def!(
    AspectFrame as "AspectFrame" |ty| {
        crate::core::from_gtype::define::<gtk4::AspectFrame, _>(&ty)?;
        ty.implements::<AspectFrame>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::AspectFrame>()?;
    }
);

pub type Assistant = crate::newtype::Gtk<gtk4::Assistant>;

crate::common::interface!(Assistant as gtk4::Assistant | _ty | {});

crate::common::class_def!(
    Assistant as "Assistant" |ty| {
        crate::core::from_gtype::define::<gtk4::Assistant, _>(&ty)?;
        ty.implements::<Assistant>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::Assistant>()?;
    }
);

pub type AssistantPage = crate::newtype::Gtk<gtk4::AssistantPage>;

crate::common::interface!(AssistantPage as gtk4::AssistantPage | _ty | {});

crate::common::class_def!(
    AssistantPage as "AssistantPage" |ty| {
        crate::core::from_gtype::define::<gtk4::AssistantPage, _>(&ty)?;
        ty.implements::<AssistantPage>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::AssistantPage>()?;
    }
);

pub type BinLayout = crate::newtype::Gtk<gtk4::BinLayout>;

crate::common::interface!(BinLayout as gtk4::BinLayout | _ty | {});

crate::common::class_def!(
    BinLayout as "BinLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::BinLayout, _>(&ty)?;
        ty.implements::<BinLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.register_as::<gtk4::BinLayout>()?;
    }
);

pub type BookmarkList = crate::newtype::Gtk<gtk4::BookmarkList>;

crate::common::interface!(BookmarkList as gtk4::BookmarkList | _ty | {});

crate::common::class_def!(
    BookmarkList as "BookmarkList" |ty| {
        crate::core::from_gtype::define::<gtk4::BookmarkList, _>(&ty)?;
        ty.implements::<BookmarkList>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::BookmarkList>()?;
    }
);

pub type BoolFilter = crate::newtype::Gtk<gtk4::BoolFilter>;

crate::common::interface!(BoolFilter as gtk4::BoolFilter | _ty | {});

crate::common::class_def!(
    BoolFilter as "BoolFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::BoolFilter, _>(&ty)?;
        ty.implements::<BoolFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.register_as::<gtk4::BoolFilter>()?;
    }
);

pub type BoxLayout = crate::newtype::Gtk<gtk4::BoxLayout>;

crate::common::interface!(BoxLayout as gtk4::BoxLayout | _ty | {});

crate::common::class_def!(
    BoxLayout as "BoxLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::BoxLayout, _>(&ty)?;
        ty.implements::<BoxLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::BoxLayout>()?;
    }
);

pub type Builder = crate::newtype::Gtk<gtk4::Builder>;

crate::common::interface!(Builder as gtk4::Builder | _ty | {});

crate::common::class_def!(
    Builder as "Builder" |ty| {
        crate::core::from_gtype::define::<gtk4::Builder, _>(&ty)?;
        ty.implements::<Builder>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Builder>()?;
    }
);

pub type BuilderListItemFactory = crate::newtype::Gtk<gtk4::BuilderListItemFactory>;

crate::common::interface!(BuilderListItemFactory as gtk4::BuilderListItemFactory | _ty | {});

crate::common::class_def!(
    BuilderListItemFactory as "BuilderListItemFactory" |ty| {
        crate::core::from_gtype::define::<gtk4::BuilderListItemFactory, _>(&ty)?;
        ty.implements::<BuilderListItemFactory>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ListItemFactory>>()?;
        ty.register_as::<gtk4::BuilderListItemFactory>()?;
    }
);

pub type Calendar = crate::newtype::Gtk<gtk4::Calendar>;

crate::common::interface!(Calendar as gtk4::Calendar | _ty | {});

crate::common::class_def!(
    Calendar as "Calendar" |ty| {
        crate::core::from_gtype::define::<gtk4::Calendar, _>(&ty)?;
        ty.implements::<Calendar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Calendar>()?;
    }
);

pub type CallbackAction = crate::newtype::Gtk<gtk4::CallbackAction>;

crate::common::interface!(CallbackAction as gtk4::CallbackAction | _ty | {});

crate::common::class_def!(
    CallbackAction as "CallbackAction" |ty| {
        crate::core::from_gtype::define::<gtk4::CallbackAction, _>(&ty)?;
        ty.implements::<CallbackAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::CallbackAction>()?;
    }
);

pub type CellArea = crate::newtype::Gtk<gtk4::CellArea>;

crate::common::interface!(CellArea as gtk4::CellArea | _ty | {});

crate::common::class_def!(
    CellArea as "CellArea" |ty| {
        crate::core::from_gtype::define::<gtk4::CellArea, _>(&ty)?;
        ty.implements::<CellArea>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.register_as::<gtk4::CellArea>()?;
    }
);

pub type CellAreaBox = crate::newtype::Gtk<gtk4::CellAreaBox>;

crate::common::interface!(CellAreaBox as gtk4::CellAreaBox | _ty | {});

crate::common::class_def!(
    CellAreaBox as "CellAreaBox" |ty| {
        crate::core::from_gtype::define::<gtk4::CellAreaBox, _>(&ty)?;
        ty.implements::<CellAreaBox>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellArea>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::CellAreaBox>()?;
    }
);

pub type CellAreaContext = crate::newtype::Gtk<gtk4::CellAreaContext>;

crate::common::interface!(CellAreaContext as gtk4::CellAreaContext | _ty | {});

crate::common::class_def!(
    CellAreaContext as "CellAreaContext" |ty| {
        crate::core::from_gtype::define::<gtk4::CellAreaContext, _>(&ty)?;
        ty.implements::<CellAreaContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::CellAreaContext>()?;
    }
);

pub type CellRenderer = crate::newtype::Gtk<gtk4::CellRenderer>;

crate::common::interface!(CellRenderer as gtk4::CellRenderer | _ty | {});

crate::common::class_def!(
    CellRenderer as "CellRenderer" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRenderer, _>(&ty)?;
        ty.implements::<CellRenderer>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRenderer>()?;
    }
);

pub type CellRendererAccel = crate::newtype::Gtk<gtk4::CellRendererAccel>;

crate::common::interface!(CellRendererAccel as gtk4::CellRendererAccel | _ty | {});

crate::common::class_def!(
    CellRendererAccel as "CellRendererAccel" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererAccel, _>(&ty)?;
        ty.implements::<CellRendererAccel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRendererText>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererAccel>()?;
    }
);

pub type CellRendererCombo = crate::newtype::Gtk<gtk4::CellRendererCombo>;

crate::common::interface!(CellRendererCombo as gtk4::CellRendererCombo | _ty | {});

crate::common::class_def!(
    CellRendererCombo as "CellRendererCombo" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererCombo, _>(&ty)?;
        ty.implements::<CellRendererCombo>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRendererText>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererCombo>()?;
    }
);

pub type CellRendererPixbuf = crate::newtype::Gtk<gtk4::CellRendererPixbuf>;

crate::common::interface!(CellRendererPixbuf as gtk4::CellRendererPixbuf | _ty | {});

crate::common::class_def!(
    CellRendererPixbuf as "CellRendererPixbuf" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererPixbuf, _>(&ty)?;
        ty.implements::<CellRendererPixbuf>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererPixbuf>()?;
    }
);

pub type CellRendererProgress = crate::newtype::Gtk<gtk4::CellRendererProgress>;

crate::common::interface!(CellRendererProgress as gtk4::CellRendererProgress | _ty | {});

crate::common::class_def!(
    CellRendererProgress as "CellRendererProgress" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererProgress, _>(&ty)?;
        ty.implements::<CellRendererProgress>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::CellRendererProgress>()?;
    }
);

pub type CellRendererSpin = crate::newtype::Gtk<gtk4::CellRendererSpin>;

crate::common::interface!(CellRendererSpin as gtk4::CellRendererSpin | _ty | {});

crate::common::class_def!(
    CellRendererSpin as "CellRendererSpin" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererSpin, _>(&ty)?;
        ty.implements::<CellRendererSpin>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRendererText>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererSpin>()?;
    }
);

pub type CellRendererSpinner = crate::newtype::Gtk<gtk4::CellRendererSpinner>;

crate::common::interface!(CellRendererSpinner as gtk4::CellRendererSpinner | _ty | {});

crate::common::class_def!(
    CellRendererSpinner as "CellRendererSpinner" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererSpinner, _>(&ty)?;
        ty.implements::<CellRendererSpinner>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererSpinner>()?;
    }
);

pub type CellRendererText = crate::newtype::Gtk<gtk4::CellRendererText>;

crate::common::interface!(CellRendererText as gtk4::CellRendererText | _ty | {});

crate::common::class_def!(
    CellRendererText as "CellRendererText" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererText, _>(&ty)?;
        ty.implements::<CellRendererText>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererText>()?;
    }
);

pub type CellRendererToggle = crate::newtype::Gtk<gtk4::CellRendererToggle>;

crate::common::interface!(CellRendererToggle as gtk4::CellRendererToggle | _ty | {});

crate::common::class_def!(
    CellRendererToggle as "CellRendererToggle" |ty| {
        crate::core::from_gtype::define::<gtk4::CellRendererToggle, _>(&ty)?;
        ty.implements::<CellRendererToggle>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellRenderer>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.register_as::<gtk4::CellRendererToggle>()?;
    }
);

pub type CellView = crate::newtype::Gtk<gtk4::CellView>;

crate::common::interface!(CellView as gtk4::CellView | _ty | {});

crate::common::class_def!(
    CellView as "CellView" |ty| {
        crate::core::from_gtype::define::<gtk4::CellView, _>(&ty)?;
        ty.implements::<CellView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::CellView>()?;
    }
);

pub type CenterBox = crate::newtype::Gtk<gtk4::CenterBox>;

crate::common::interface!(CenterBox as gtk4::CenterBox | _ty | {});

crate::common::class_def!(
    CenterBox as "CenterBox" |ty| {
        crate::core::from_gtype::define::<gtk4::CenterBox, _>(&ty)?;
        ty.implements::<CenterBox>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::CenterBox>()?;
    }
);

pub type CenterLayout = crate::newtype::Gtk<gtk4::CenterLayout>;

crate::common::interface!(CenterLayout as gtk4::CenterLayout | _ty | {});

crate::common::class_def!(
    CenterLayout as "CenterLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::CenterLayout, _>(&ty)?;
        ty.implements::<CenterLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.register_as::<gtk4::CenterLayout>()?;
    }
);

pub type CheckButton = crate::newtype::Gtk<gtk4::CheckButton>;

crate::common::interface!(CheckButton as gtk4::CheckButton | _ty | {});

crate::common::class_def!(
    CheckButton as "CheckButton" |ty| {
        crate::core::from_gtype::define::<gtk4::CheckButton, _>(&ty)?;
        ty.implements::<CheckButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::CheckButton>()?;
    }
);

pub type ColorButton = crate::newtype::Gtk<gtk4::ColorButton>;

crate::common::interface!(ColorButton as gtk4::ColorButton | _ty | {});

crate::common::class_def!(
    ColorButton as "ColorButton" |ty| {
        crate::core::from_gtype::define::<gtk4::ColorButton, _>(&ty)?;
        ty.implements::<ColorButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ColorChooser>>()?;
        ty.register_as::<gtk4::ColorButton>()?;
    }
);

pub type ColorChooserDialog = crate::newtype::Gtk<gtk4::ColorChooserDialog>;

crate::common::interface!(ColorChooserDialog as gtk4::ColorChooserDialog | _ty | {});

crate::common::class_def!(
    ColorChooserDialog as "ColorChooserDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::ColorChooserDialog, _>(&ty)?;
        ty.implements::<ColorChooserDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ColorChooser>>()?;
        ty.register_as::<gtk4::ColorChooserDialog>()?;
    }
);

pub type ColorChooserWidget = crate::newtype::Gtk<gtk4::ColorChooserWidget>;

crate::common::interface!(ColorChooserWidget as gtk4::ColorChooserWidget | _ty | {});

crate::common::class_def!(
    ColorChooserWidget as "ColorChooserWidget" |ty| {
        crate::core::from_gtype::define::<gtk4::ColorChooserWidget, _>(&ty)?;
        ty.implements::<ColorChooserWidget>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ColorChooser>>()?;
        ty.register_as::<gtk4::ColorChooserWidget>()?;
    }
);

pub type ColumnView = crate::newtype::Gtk<gtk4::ColumnView>;

crate::common::interface!(ColumnView as gtk4::ColumnView | _ty | {});

crate::common::class_def!(
    ColumnView as "ColumnView" |ty| {
        crate::core::from_gtype::define::<gtk4::ColumnView, _>(&ty)?;
        ty.implements::<ColumnView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::ColumnView>()?;
    }
);

pub type ColumnViewColumn = crate::newtype::Gtk<gtk4::ColumnViewColumn>;

crate::common::interface!(ColumnViewColumn as gtk4::ColumnViewColumn | _ty | {});

crate::common::class_def!(
    ColumnViewColumn as "ColumnViewColumn" |ty| {
        crate::core::from_gtype::define::<gtk4::ColumnViewColumn, _>(&ty)?;
        ty.implements::<ColumnViewColumn>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ColumnViewColumn>()?;
    }
);

pub type ComboBox = crate::newtype::Gtk<gtk4::ComboBox>;

crate::common::interface!(ComboBox as gtk4::ComboBox | _ty | {});

crate::common::class_def!(
    ComboBox as "ComboBox" |ty| {
        crate::core::from_gtype::define::<gtk4::ComboBox, _>(&ty)?;
        ty.implements::<ComboBox>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellEditable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.register_as::<gtk4::ComboBox>()?;
    }
);

pub type ComboBoxText = crate::newtype::Gtk<gtk4::ComboBoxText>;

crate::common::interface!(ComboBoxText as gtk4::ComboBoxText | _ty | {});

crate::common::class_def!(
    ComboBoxText as "ComboBoxText" |ty| {
        crate::core::from_gtype::define::<gtk4::ComboBoxText, _>(&ty)?;
        ty.implements::<ComboBoxText>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ComboBox>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellEditable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.register_as::<gtk4::ComboBoxText>()?;
    }
);

pub type Constraint = crate::newtype::Gtk<gtk4::Constraint>;

crate::common::interface!(Constraint as gtk4::Constraint | _ty | {});

crate::common::class_def!(
    Constraint as "Constraint" |ty| {
        crate::core::from_gtype::define::<gtk4::Constraint, _>(&ty)?;
        ty.implements::<Constraint>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Constraint>()?;
    }
);

pub type ConstraintGuide = crate::newtype::Gtk<gtk4::ConstraintGuide>;

crate::common::interface!(ConstraintGuide as gtk4::ConstraintGuide | _ty | {});

crate::common::class_def!(
    ConstraintGuide as "ConstraintGuide" |ty| {
        crate::core::from_gtype::define::<gtk4::ConstraintGuide, _>(&ty)?;
        ty.implements::<ConstraintGuide>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ConstraintGuide>()?;
    }
);

pub type ConstraintLayout = crate::newtype::Gtk<gtk4::ConstraintLayout>;

crate::common::interface!(ConstraintLayout as gtk4::ConstraintLayout | _ty | {});

crate::common::class_def!(
    ConstraintLayout as "ConstraintLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::ConstraintLayout, _>(&ty)?;
        ty.implements::<ConstraintLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::ConstraintLayout>()?;
    }
);

pub type ConstraintLayoutChild = crate::newtype::Gtk<gtk4::ConstraintLayoutChild>;

crate::common::interface!(ConstraintLayoutChild as gtk4::ConstraintLayoutChild | _ty | {});

crate::common::class_def!(
    ConstraintLayoutChild as "ConstraintLayoutChild" |ty| {
        crate::core::from_gtype::define::<gtk4::ConstraintLayoutChild, _>(&ty)?;
        ty.implements::<ConstraintLayoutChild>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutChild>>()?;
        ty.register_as::<gtk4::ConstraintLayoutChild>()?;
    }
);

pub type CustomFilter = crate::newtype::Gtk<gtk4::CustomFilter>;

crate::common::interface!(CustomFilter as gtk4::CustomFilter | _ty | {});

crate::common::class_def!(
    CustomFilter as "CustomFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::CustomFilter, _>(&ty)?;
        ty.implements::<CustomFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.register_as::<gtk4::CustomFilter>()?;
    }
);

pub type CustomSorter = crate::newtype::Gtk<gtk4::CustomSorter>;

crate::common::interface!(CustomSorter as gtk4::CustomSorter | _ty | {});

crate::common::class_def!(
    CustomSorter as "CustomSorter" |ty| {
        crate::core::from_gtype::define::<gtk4::CustomSorter, _>(&ty)?;
        ty.implements::<CustomSorter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Sorter>>()?;
        ty.register_as::<gtk4::CustomSorter>()?;
    }
);

pub type Dialog = crate::newtype::Gtk<gtk4::Dialog>;

crate::common::interface!(Dialog as gtk4::Dialog | _ty | {});

crate::common::class_def!(
    Dialog as "Dialog" |ty| {
        crate::core::from_gtype::define::<gtk4::Dialog, _>(&ty)?;
        ty.implements::<Dialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::Dialog>()?;
    }
);

pub type DirectoryList = crate::newtype::Gtk<gtk4::DirectoryList>;

crate::common::interface!(DirectoryList as gtk4::DirectoryList | _ty | {});

crate::common::class_def!(
    DirectoryList as "DirectoryList" |ty| {
        crate::core::from_gtype::define::<gtk4::DirectoryList, _>(&ty)?;
        ty.implements::<DirectoryList>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::DirectoryList>()?;
    }
);

pub type DragIcon = crate::newtype::Gtk<gtk4::DragIcon>;

crate::common::interface!(DragIcon as gtk4::DragIcon | _ty | {});

crate::common::class_def!(
    DragIcon as "DragIcon" |ty| {
        crate::core::from_gtype::define::<gtk4::DragIcon, _>(&ty)?;
        ty.implements::<DragIcon>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.register_as::<gtk4::DragIcon>()?;
    }
);

pub type DragSource = crate::newtype::Gtk<gtk4::DragSource>;

crate::common::interface!(DragSource as gtk4::DragSource | _ty | {});

crate::common::class_def!(
    DragSource as "DragSource" |ty| {
        crate::core::from_gtype::define::<gtk4::DragSource, _>(&ty)?;
        ty.implements::<DragSource>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::DragSource>()?;
    }
);

pub type DrawingArea = crate::newtype::Gtk<gtk4::DrawingArea>;

crate::common::interface!(DrawingArea as gtk4::DrawingArea | _ty | {});

crate::common::class_def!(
    DrawingArea as "DrawingArea" |ty| {
        crate::core::from_gtype::define::<gtk4::DrawingArea, _>(&ty)?;
        ty.implements::<DrawingArea>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::DrawingArea>()?;
    }
);

pub type DropControllerMotion = crate::newtype::Gtk<gtk4::DropControllerMotion>;

crate::common::interface!(DropControllerMotion as gtk4::DropControllerMotion | _ty | {});

crate::common::class_def!(
    DropControllerMotion as "DropControllerMotion" |ty| {
        crate::core::from_gtype::define::<gtk4::DropControllerMotion, _>(&ty)?;
        ty.implements::<DropControllerMotion>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::DropControllerMotion>()?;
    }
);

pub type DropDown = crate::newtype::Gtk<gtk4::DropDown>;

crate::common::interface!(DropDown as gtk4::DropDown | _ty | {});

crate::common::class_def!(
    DropDown as "DropDown" |ty| {
        crate::core::from_gtype::define::<gtk4::DropDown, _>(&ty)?;
        ty.implements::<DropDown>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::DropDown>()?;
    }
);

pub type DropTarget = crate::newtype::Gtk<gtk4::DropTarget>;

crate::common::interface!(DropTarget as gtk4::DropTarget | _ty | {});

crate::common::class_def!(
    DropTarget as "DropTarget" |ty| {
        crate::core::from_gtype::define::<gtk4::DropTarget, _>(&ty)?;
        ty.implements::<DropTarget>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::DropTarget>()?;
    }
);

pub type DropTargetAsync = crate::newtype::Gtk<gtk4::DropTargetAsync>;

crate::common::interface!(DropTargetAsync as gtk4::DropTargetAsync | _ty | {});

crate::common::class_def!(
    DropTargetAsync as "DropTargetAsync" |ty| {
        crate::core::from_gtype::define::<gtk4::DropTargetAsync, _>(&ty)?;
        ty.implements::<DropTargetAsync>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::DropTargetAsync>()?;
    }
);

pub type EditableLabel = crate::newtype::Gtk<gtk4::EditableLabel>;

crate::common::interface!(EditableLabel as gtk4::EditableLabel | _ty | {});

crate::common::class_def!(
    EditableLabel as "EditableLabel" |ty| {
        crate::core::from_gtype::define::<gtk4::EditableLabel, _>(&ty)?;
        ty.implements::<EditableLabel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.register_as::<gtk4::EditableLabel>()?;
    }
);

pub type EmojiChooser = crate::newtype::Gtk<gtk4::EmojiChooser>;

crate::common::interface!(EmojiChooser as gtk4::EmojiChooser | _ty | {});

crate::common::class_def!(
    EmojiChooser as "EmojiChooser" |ty| {
        crate::core::from_gtype::define::<gtk4::EmojiChooser, _>(&ty)?;
        ty.implements::<EmojiChooser>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Popover>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::EmojiChooser>()?;
    }
);

pub type Entry = crate::newtype::Gtk<gtk4::Entry>;

crate::common::interface!(Entry as gtk4::Entry | _ty | {});

crate::common::class_def!(
    Entry as "Entry" |ty| {
        crate::core::from_gtype::define::<gtk4::Entry, _>(&ty)?;
        ty.implements::<Entry>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellEditable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.register_as::<gtk4::Entry>()?;
    }
);

pub type EntryBuffer = crate::newtype::Gtk<gtk4::EntryBuffer>;

crate::common::interface!(EntryBuffer as gtk4::EntryBuffer | _ty | {});

crate::common::class_def!(
    EntryBuffer as "EntryBuffer" |ty| {
        crate::core::from_gtype::define::<gtk4::EntryBuffer, _>(&ty)?;
        ty.implements::<EntryBuffer>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::EntryBuffer>()?;
    }
);

pub type EntryCompletion = crate::newtype::Gtk<gtk4::EntryCompletion>;

crate::common::interface!(EntryCompletion as gtk4::EntryCompletion | _ty | {});

crate::common::class_def!(
    EntryCompletion as "EntryCompletion" |ty| {
        crate::core::from_gtype::define::<gtk4::EntryCompletion, _>(&ty)?;
        ty.implements::<EntryCompletion>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.register_as::<gtk4::EntryCompletion>()?;
    }
);

pub type EventController = crate::newtype::Gtk<gtk4::EventController>;

crate::common::interface!(EventController as gtk4::EventController | _ty | {});

crate::common::class_def!(
    EventController as "EventController" |ty| {
        crate::core::from_gtype::define::<gtk4::EventController, _>(&ty)?;
        ty.implements::<EventController>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::EventController>()?;
    }
);

pub type EventControllerFocus = crate::newtype::Gtk<gtk4::EventControllerFocus>;

crate::common::interface!(EventControllerFocus as gtk4::EventControllerFocus | _ty | {});

crate::common::class_def!(
    EventControllerFocus as "EventControllerFocus" |ty| {
        crate::core::from_gtype::define::<gtk4::EventControllerFocus, _>(&ty)?;
        ty.implements::<EventControllerFocus>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::EventControllerFocus>()?;
    }
);

pub type EventControllerKey = crate::newtype::Gtk<gtk4::EventControllerKey>;

crate::common::interface!(EventControllerKey as gtk4::EventControllerKey | _ty | {});

crate::common::class_def!(
    EventControllerKey as "EventControllerKey" |ty| {
        crate::core::from_gtype::define::<gtk4::EventControllerKey, _>(&ty)?;
        ty.implements::<EventControllerKey>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::EventControllerKey>()?;
    }
);

pub type EventControllerLegacy = crate::newtype::Gtk<gtk4::EventControllerLegacy>;

crate::common::interface!(EventControllerLegacy as gtk4::EventControllerLegacy | _ty | {});

crate::common::class_def!(
    EventControllerLegacy as "EventControllerLegacy" |ty| {
        crate::core::from_gtype::define::<gtk4::EventControllerLegacy, _>(&ty)?;
        ty.implements::<EventControllerLegacy>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::EventControllerLegacy>()?;
    }
);

pub type EventControllerMotion = crate::newtype::Gtk<gtk4::EventControllerMotion>;

crate::common::interface!(EventControllerMotion as gtk4::EventControllerMotion | _ty | {});

crate::common::class_def!(
    EventControllerMotion as "EventControllerMotion" |ty| {
        crate::core::from_gtype::define::<gtk4::EventControllerMotion, _>(&ty)?;
        ty.implements::<EventControllerMotion>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::EventControllerMotion>()?;
    }
);

pub type EventControllerScroll = crate::newtype::Gtk<gtk4::EventControllerScroll>;

crate::common::interface!(EventControllerScroll as gtk4::EventControllerScroll | _ty | {});

crate::common::class_def!(
    EventControllerScroll as "EventControllerScroll" |ty| {
        crate::core::from_gtype::define::<gtk4::EventControllerScroll, _>(&ty)?;
        ty.implements::<EventControllerScroll>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::EventControllerScroll>()?;
    }
);

pub type EveryFilter = crate::newtype::Gtk<gtk4::EveryFilter>;

crate::common::interface!(EveryFilter as gtk4::EveryFilter | _ty | {});

crate::common::class_def!(
    EveryFilter as "EveryFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::EveryFilter, _>(&ty)?;
        ty.implements::<EveryFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::MultiFilter>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::EveryFilter>()?;
    }
);

pub type Expander = crate::newtype::Gtk<gtk4::Expander>;

crate::common::interface!(Expander as gtk4::Expander | _ty | {});

crate::common::class_def!(
    Expander as "Expander" |ty| {
        crate::core::from_gtype::define::<gtk4::Expander, _>(&ty)?;
        ty.implements::<Expander>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Expander>()?;
    }
);

pub type FileChooserDialog = crate::newtype::Gtk<gtk4::FileChooserDialog>;

crate::common::interface!(FileChooserDialog as gtk4::FileChooserDialog | _ty | {});

crate::common::class_def!(
    FileChooserDialog as "FileChooserDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::FileChooserDialog, _>(&ty)?;
        ty.implements::<FileChooserDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FileChooser>>()?;
        ty.register_as::<gtk4::FileChooserDialog>()?;
    }
);

pub type FileChooserNative = crate::newtype::Gtk<gtk4::FileChooserNative>;

crate::common::interface!(FileChooserNative as gtk4::FileChooserNative | _ty | {});

crate::common::class_def!(
    FileChooserNative as "FileChooserNative" |ty| {
        crate::core::from_gtype::define::<gtk4::FileChooserNative, _>(&ty)?;
        ty.implements::<FileChooserNative>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::NativeDialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FileChooser>>()?;
        ty.register_as::<gtk4::FileChooserNative>()?;
    }
);

pub type FileChooserWidget = crate::newtype::Gtk<gtk4::FileChooserWidget>;

crate::common::interface!(FileChooserWidget as gtk4::FileChooserWidget | _ty | {});

crate::common::class_def!(
    FileChooserWidget as "FileChooserWidget" |ty| {
        crate::core::from_gtype::define::<gtk4::FileChooserWidget, _>(&ty)?;
        ty.implements::<FileChooserWidget>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FileChooser>>()?;
        ty.register_as::<gtk4::FileChooserWidget>()?;
    }
);

pub type FileFilter = crate::newtype::Gtk<gtk4::FileFilter>;

crate::common::interface!(FileFilter as gtk4::FileFilter | _ty | {});

crate::common::class_def!(
    FileFilter as "FileFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::FileFilter, _>(&ty)?;
        ty.implements::<FileFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::FileFilter>()?;
    }
);

pub type Filter = crate::newtype::Gtk<gtk4::Filter>;

crate::common::interface!(Filter as gtk4::Filter | _ty | {});

crate::common::class_def!(
    Filter as "Filter" |ty| {
        crate::core::from_gtype::define::<gtk4::Filter, _>(&ty)?;
        ty.implements::<Filter>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Filter>()?;
    }
);

pub type FilterListModel = crate::newtype::Gtk<gtk4::FilterListModel>;

crate::common::interface!(FilterListModel as gtk4::FilterListModel | _ty | {});

crate::common::class_def!(
    FilterListModel as "FilterListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::FilterListModel, _>(&ty)?;
        ty.implements::<FilterListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::FilterListModel>()?;
    }
);

pub type Fixed = crate::newtype::Gtk<gtk4::Fixed>;

crate::common::interface!(Fixed as gtk4::Fixed | _ty | {});

crate::common::class_def!(
    Fixed as "Fixed" |ty| {
        crate::core::from_gtype::define::<gtk4::Fixed, _>(&ty)?;
        ty.implements::<Fixed>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Fixed>()?;
    }
);

pub type FixedLayout = crate::newtype::Gtk<gtk4::FixedLayout>;

crate::common::interface!(FixedLayout as gtk4::FixedLayout | _ty | {});

crate::common::class_def!(
    FixedLayout as "FixedLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::FixedLayout, _>(&ty)?;
        ty.implements::<FixedLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.register_as::<gtk4::FixedLayout>()?;
    }
);

pub type FixedLayoutChild = crate::newtype::Gtk<gtk4::FixedLayoutChild>;

crate::common::interface!(FixedLayoutChild as gtk4::FixedLayoutChild | _ty | {});

crate::common::class_def!(
    FixedLayoutChild as "FixedLayoutChild" |ty| {
        crate::core::from_gtype::define::<gtk4::FixedLayoutChild, _>(&ty)?;
        ty.implements::<FixedLayoutChild>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutChild>>()?;
        ty.register_as::<gtk4::FixedLayoutChild>()?;
    }
);

pub type FlattenListModel = crate::newtype::Gtk<gtk4::FlattenListModel>;

crate::common::interface!(FlattenListModel as gtk4::FlattenListModel | _ty | {});

crate::common::class_def!(
    FlattenListModel as "FlattenListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::FlattenListModel, _>(&ty)?;
        ty.implements::<FlattenListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::FlattenListModel>()?;
    }
);

pub type FlowBox = crate::newtype::Gtk<gtk4::FlowBox>;

crate::common::interface!(FlowBox as gtk4::FlowBox | _ty | {});

crate::common::class_def!(
    FlowBox as "FlowBox" |ty| {
        crate::core::from_gtype::define::<gtk4::FlowBox, _>(&ty)?;
        ty.implements::<FlowBox>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::FlowBox>()?;
    }
);

pub type FlowBoxChild = crate::newtype::Gtk<gtk4::FlowBoxChild>;

crate::common::interface!(FlowBoxChild as gtk4::FlowBoxChild | _ty | {});

crate::common::class_def!(
    FlowBoxChild as "FlowBoxChild" |ty| {
        crate::core::from_gtype::define::<gtk4::FlowBoxChild, _>(&ty)?;
        ty.implements::<FlowBoxChild>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::FlowBoxChild>()?;
    }
);

pub type FontButton = crate::newtype::Gtk<gtk4::FontButton>;

crate::common::interface!(FontButton as gtk4::FontButton | _ty | {});

crate::common::class_def!(
    FontButton as "FontButton" |ty| {
        crate::core::from_gtype::define::<gtk4::FontButton, _>(&ty)?;
        ty.implements::<FontButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FontChooser>>()?;
        ty.register_as::<gtk4::FontButton>()?;
    }
);

pub type FontChooserDialog = crate::newtype::Gtk<gtk4::FontChooserDialog>;

crate::common::interface!(FontChooserDialog as gtk4::FontChooserDialog | _ty | {});

crate::common::class_def!(
    FontChooserDialog as "FontChooserDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::FontChooserDialog, _>(&ty)?;
        ty.implements::<FontChooserDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FontChooser>>()?;
        ty.register_as::<gtk4::FontChooserDialog>()?;
    }
);

pub type FontChooserWidget = crate::newtype::Gtk<gtk4::FontChooserWidget>;

crate::common::interface!(FontChooserWidget as gtk4::FontChooserWidget | _ty | {});

crate::common::class_def!(
    FontChooserWidget as "FontChooserWidget" |ty| {
        crate::core::from_gtype::define::<gtk4::FontChooserWidget, _>(&ty)?;
        ty.implements::<FontChooserWidget>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::FontChooser>>()?;
        ty.register_as::<gtk4::FontChooserWidget>()?;
    }
);

pub type Frame = crate::newtype::Gtk<gtk4::Frame>;

crate::common::interface!(Frame as gtk4::Frame | _ty | {});

crate::common::class_def!(
    Frame as "Frame" |ty| {
        crate::core::from_gtype::define::<gtk4::Frame, _>(&ty)?;
        ty.implements::<Frame>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Frame>()?;
    }
);

pub type GLArea = crate::newtype::Gtk<gtk4::GLArea>;

crate::common::interface!(GLArea as gtk4::GLArea | _ty | {});

crate::common::class_def!(
    GLArea as "GLArea" |ty| {
        crate::core::from_gtype::define::<gtk4::GLArea, _>(&ty)?;
        ty.implements::<GLArea>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::GLArea>()?;
    }
);

pub type Gesture = crate::newtype::Gtk<gtk4::Gesture>;

crate::common::interface!(Gesture as gtk4::Gesture | _ty | {});

crate::common::class_def!(
    Gesture as "Gesture" |ty| {
        crate::core::from_gtype::define::<gtk4::Gesture, _>(&ty)?;
        ty.implements::<Gesture>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::Gesture>()?;
    }
);

pub type GestureClick = crate::newtype::Gtk<gtk4::GestureClick>;

crate::common::interface!(GestureClick as gtk4::GestureClick | _ty | {});

crate::common::class_def!(
    GestureClick as "GestureClick" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureClick, _>(&ty)?;
        ty.implements::<GestureClick>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureClick>()?;
    }
);

pub type GestureDrag = crate::newtype::Gtk<gtk4::GestureDrag>;

crate::common::interface!(GestureDrag as gtk4::GestureDrag | _ty | {});

crate::common::class_def!(
    GestureDrag as "GestureDrag" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureDrag, _>(&ty)?;
        ty.implements::<GestureDrag>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureDrag>()?;
    }
);

pub type GestureLongPress = crate::newtype::Gtk<gtk4::GestureLongPress>;

crate::common::interface!(GestureLongPress as gtk4::GestureLongPress | _ty | {});

crate::common::class_def!(
    GestureLongPress as "GestureLongPress" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureLongPress, _>(&ty)?;
        ty.implements::<GestureLongPress>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureLongPress>()?;
    }
);

pub type GesturePan = crate::newtype::Gtk<gtk4::GesturePan>;

crate::common::interface!(GesturePan as gtk4::GesturePan | _ty | {});

crate::common::class_def!(
    GesturePan as "GesturePan" |ty| {
        crate::core::from_gtype::define::<gtk4::GesturePan, _>(&ty)?;
        ty.implements::<GesturePan>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureDrag>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GesturePan>()?;
    }
);

pub type GestureRotate = crate::newtype::Gtk<gtk4::GestureRotate>;

crate::common::interface!(GestureRotate as gtk4::GestureRotate | _ty | {});

crate::common::class_def!(
    GestureRotate as "GestureRotate" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureRotate, _>(&ty)?;
        ty.implements::<GestureRotate>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureRotate>()?;
    }
);

pub type GestureSingle = crate::newtype::Gtk<gtk4::GestureSingle>;

crate::common::interface!(GestureSingle as gtk4::GestureSingle | _ty | {});

crate::common::class_def!(
    GestureSingle as "GestureSingle" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureSingle, _>(&ty)?;
        ty.implements::<GestureSingle>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureSingle>()?;
    }
);

pub type GestureStylus = crate::newtype::Gtk<gtk4::GestureStylus>;

crate::common::interface!(GestureStylus as gtk4::GestureStylus | _ty | {});

crate::common::class_def!(
    GestureStylus as "GestureStylus" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureStylus, _>(&ty)?;
        ty.implements::<GestureStylus>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureStylus>()?;
    }
);

pub type GestureSwipe = crate::newtype::Gtk<gtk4::GestureSwipe>;

crate::common::interface!(GestureSwipe as gtk4::GestureSwipe | _ty | {});

crate::common::class_def!(
    GestureSwipe as "GestureSwipe" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureSwipe, _>(&ty)?;
        ty.implements::<GestureSwipe>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::GestureSingle>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureSwipe>()?;
    }
);

pub type GestureZoom = crate::newtype::Gtk<gtk4::GestureZoom>;

crate::common::interface!(GestureZoom as gtk4::GestureZoom | _ty | {});

crate::common::class_def!(
    GestureZoom as "GestureZoom" |ty| {
        crate::core::from_gtype::define::<gtk4::GestureZoom, _>(&ty)?;
        ty.implements::<GestureZoom>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Gesture>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::GestureZoom>()?;
    }
);

pub type Grid = crate::newtype::Gtk<gtk4::Grid>;

crate::common::interface!(Grid as gtk4::Grid | _ty | {});

crate::common::class_def!(
    Grid as "Grid" |ty| {
        crate::core::from_gtype::define::<gtk4::Grid, _>(&ty)?;
        ty.implements::<Grid>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Grid>()?;
    }
);

pub type GridLayout = crate::newtype::Gtk<gtk4::GridLayout>;

crate::common::interface!(GridLayout as gtk4::GridLayout | _ty | {});

crate::common::class_def!(
    GridLayout as "GridLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::GridLayout, _>(&ty)?;
        ty.implements::<GridLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.register_as::<gtk4::GridLayout>()?;
    }
);

pub type GridLayoutChild = crate::newtype::Gtk<gtk4::GridLayoutChild>;

crate::common::interface!(GridLayoutChild as gtk4::GridLayoutChild | _ty | {});

crate::common::class_def!(
    GridLayoutChild as "GridLayoutChild" |ty| {
        crate::core::from_gtype::define::<gtk4::GridLayoutChild, _>(&ty)?;
        ty.implements::<GridLayoutChild>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutChild>>()?;
        ty.register_as::<gtk4::GridLayoutChild>()?;
    }
);

pub type GridView = crate::newtype::Gtk<gtk4::GridView>;

crate::common::interface!(GridView as gtk4::GridView | _ty | {});

crate::common::class_def!(
    GridView as "GridView" |ty| {
        crate::core::from_gtype::define::<gtk4::GridView, _>(&ty)?;
        ty.implements::<GridView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ListBase>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::GridView>()?;
    }
);

pub type HeaderBar = crate::newtype::Gtk<gtk4::HeaderBar>;

crate::common::interface!(HeaderBar as gtk4::HeaderBar | _ty | {});

crate::common::class_def!(
    HeaderBar as "HeaderBar" |ty| {
        crate::core::from_gtype::define::<gtk4::HeaderBar, _>(&ty)?;
        ty.implements::<HeaderBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::HeaderBar>()?;
    }
);

pub type IMContext = crate::newtype::Gtk<gtk4::IMContext>;

crate::common::interface!(IMContext as gtk4::IMContext | _ty | {});

crate::common::class_def!(
    IMContext as "IMContext" |ty| {
        crate::core::from_gtype::define::<gtk4::IMContext, _>(&ty)?;
        ty.implements::<IMContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::IMContext>()?;
    }
);

pub type IMContextSimple = crate::newtype::Gtk<gtk4::IMContextSimple>;

crate::common::interface!(IMContextSimple as gtk4::IMContextSimple | _ty | {});

crate::common::class_def!(
    IMContextSimple as "IMContextSimple" |ty| {
        crate::core::from_gtype::define::<gtk4::IMContextSimple, _>(&ty)?;
        ty.implements::<IMContextSimple>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::IMContext>>()?;
        ty.register_as::<gtk4::IMContextSimple>()?;
    }
);

pub type IMMulticontext = crate::newtype::Gtk<gtk4::IMMulticontext>;

crate::common::interface!(IMMulticontext as gtk4::IMMulticontext | _ty | {});

crate::common::class_def!(
    IMMulticontext as "IMMulticontext" |ty| {
        crate::core::from_gtype::define::<gtk4::IMMulticontext, _>(&ty)?;
        ty.implements::<IMMulticontext>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::IMContext>>()?;
        ty.register_as::<gtk4::IMMulticontext>()?;
    }
);

pub type IconPaintable = crate::newtype::Gtk<gtk4::IconPaintable>;

crate::common::interface!(IconPaintable as gtk4::IconPaintable | _ty | {});

crate::common::class_def!(
    IconPaintable as "IconPaintable" |ty| {
        crate::core::from_gtype::define::<gtk4::IconPaintable, _>(&ty)?;
        ty.implements::<IconPaintable>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gtk4::IconPaintable>()?;
    }
);

pub type IconTheme = crate::newtype::Gtk<gtk4::IconTheme>;

crate::common::interface!(IconTheme as gtk4::IconTheme | _ty | {});

crate::common::class_def!(
    IconTheme as "IconTheme" |ty| {
        crate::core::from_gtype::define::<gtk4::IconTheme, _>(&ty)?;
        ty.implements::<IconTheme>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::IconTheme>()?;
    }
);

pub type IconView = crate::newtype::Gtk<gtk4::IconView>;

crate::common::interface!(IconView as gtk4::IconView | _ty | {});

crate::common::class_def!(
    IconView as "IconView" |ty| {
        crate::core::from_gtype::define::<gtk4::IconView, _>(&ty)?;
        ty.implements::<IconView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::IconView>()?;
    }
);

pub type Image = crate::newtype::Gtk<gtk4::Image>;

crate::common::interface!(Image as gtk4::Image | _ty | {});

crate::common::class_def!(
    Image as "Image" |ty| {
        crate::core::from_gtype::define::<gtk4::Image, _>(&ty)?;
        ty.implements::<Image>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Image>()?;
    }
);

pub type InfoBar = crate::newtype::Gtk<gtk4::InfoBar>;

crate::common::interface!(InfoBar as gtk4::InfoBar | _ty | {});

crate::common::class_def!(
    InfoBar as "InfoBar" |ty| {
        crate::core::from_gtype::define::<gtk4::InfoBar, _>(&ty)?;
        ty.implements::<InfoBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::InfoBar>()?;
    }
);

pub type KeyvalTrigger = crate::newtype::Gtk<gtk4::KeyvalTrigger>;

crate::common::interface!(KeyvalTrigger as gtk4::KeyvalTrigger | _ty | {});

crate::common::class_def!(
    KeyvalTrigger as "KeyvalTrigger" |ty| {
        crate::core::from_gtype::define::<gtk4::KeyvalTrigger, _>(&ty)?;
        ty.implements::<KeyvalTrigger>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutTrigger>>()?;
        ty.register_as::<gtk4::KeyvalTrigger>()?;
    }
);

pub type Label = crate::newtype::Gtk<gtk4::Label>;

crate::common::interface!(Label as gtk4::Label | _ty | {});

crate::common::class_def!(
    Label as "Label" |ty| {
        crate::core::from_gtype::define::<gtk4::Label, _>(&ty)?;
        ty.implements::<Label>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Label>()?;
    }
);

pub type LayoutChild = crate::newtype::Gtk<gtk4::LayoutChild>;

crate::common::interface!(LayoutChild as gtk4::LayoutChild | _ty | {});

crate::common::class_def!(
    LayoutChild as "LayoutChild" |ty| {
        crate::core::from_gtype::define::<gtk4::LayoutChild, _>(&ty)?;
        ty.implements::<LayoutChild>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::LayoutChild>()?;
    }
);

pub type LayoutManager = crate::newtype::Gtk<gtk4::LayoutManager>;

crate::common::interface!(LayoutManager as gtk4::LayoutManager | _ty | {});

crate::common::class_def!(
    LayoutManager as "LayoutManager" |ty| {
        crate::core::from_gtype::define::<gtk4::LayoutManager, _>(&ty)?;
        ty.implements::<LayoutManager>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::LayoutManager>()?;
    }
);

pub type LevelBar = crate::newtype::Gtk<gtk4::LevelBar>;

crate::common::interface!(LevelBar as gtk4::LevelBar | _ty | {});

crate::common::class_def!(
    LevelBar as "LevelBar" |ty| {
        crate::core::from_gtype::define::<gtk4::LevelBar, _>(&ty)?;
        ty.implements::<LevelBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::LevelBar>()?;
    }
);

pub type LinkButton = crate::newtype::Gtk<gtk4::LinkButton>;

crate::common::interface!(LinkButton as gtk4::LinkButton | _ty | {});

crate::common::class_def!(
    LinkButton as "LinkButton" |ty| {
        crate::core::from_gtype::define::<gtk4::LinkButton, _>(&ty)?;
        ty.implements::<LinkButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Button>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::LinkButton>()?;
    }
);

pub type ListBase = crate::newtype::Gtk<gtk4::ListBase>;

crate::common::interface!(ListBase as gtk4::ListBase | _ty | {});

crate::common::class_def!(
    ListBase as "ListBase" |ty| {
        crate::core::from_gtype::define::<gtk4::ListBase, _>(&ty)?;
        ty.implements::<ListBase>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::ListBase>()?;
    }
);

pub type ListBox = crate::newtype::Gtk<gtk4::ListBox>;

crate::common::interface!(ListBox as gtk4::ListBox | _ty | {});

crate::common::class_def!(
    ListBox as "ListBox" |ty| {
        crate::core::from_gtype::define::<gtk4::ListBox, _>(&ty)?;
        ty.implements::<ListBox>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ListBox>()?;
    }
);

pub type ListBoxRow = crate::newtype::Gtk<gtk4::ListBoxRow>;

crate::common::interface!(ListBoxRow as gtk4::ListBoxRow | _ty | {});

crate::common::class_def!(
    ListBoxRow as "ListBoxRow" |ty| {
        crate::core::from_gtype::define::<gtk4::ListBoxRow, _>(&ty)?;
        ty.implements::<ListBoxRow>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::ListBoxRow>()?;
    }
);

pub type ListItem = crate::newtype::Gtk<gtk4::ListItem>;

crate::common::interface!(ListItem as gtk4::ListItem | _ty | {});

crate::common::class_def!(
    ListItem as "ListItem" |ty| {
        crate::core::from_gtype::define::<gtk4::ListItem, _>(&ty)?;
        ty.implements::<ListItem>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ListItem>()?;
    }
);

pub type ListItemFactory = crate::newtype::Gtk<gtk4::ListItemFactory>;

crate::common::interface!(ListItemFactory as gtk4::ListItemFactory | _ty | {});

crate::common::class_def!(
    ListItemFactory as "ListItemFactory" |ty| {
        crate::core::from_gtype::define::<gtk4::ListItemFactory, _>(&ty)?;
        ty.implements::<ListItemFactory>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ListItemFactory>()?;
    }
);

pub type ListStore = crate::newtype::Gtk<gtk4::ListStore>;

crate::common::interface!(ListStore as gtk4::ListStore | _ty | {});

crate::common::class_def!(
    ListStore as "ListStore" |ty| {
        crate::core::from_gtype::define::<gtk4::ListStore, _>(&ty)?;
        ty.implements::<ListStore>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragDest>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragSource>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeSortable>>()?;
        ty.register_as::<gtk4::ListStore>()?;
    }
);

pub type ListView = crate::newtype::Gtk<gtk4::ListView>;

crate::common::interface!(ListView as gtk4::ListView | _ty | {});

crate::common::class_def!(
    ListView as "ListView" |ty| {
        crate::core::from_gtype::define::<gtk4::ListView, _>(&ty)?;
        ty.implements::<ListView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ListBase>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::ListView>()?;
    }
);

pub type LockButton = crate::newtype::Gtk<gtk4::LockButton>;

crate::common::interface!(LockButton as gtk4::LockButton | _ty | {});

crate::common::class_def!(
    LockButton as "LockButton" |ty| {
        crate::core::from_gtype::define::<gtk4::LockButton, _>(&ty)?;
        ty.implements::<LockButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Button>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::LockButton>()?;
    }
);

pub type MapListModel = crate::newtype::Gtk<gtk4::MapListModel>;

crate::common::interface!(MapListModel as gtk4::MapListModel | _ty | {});

crate::common::class_def!(
    MapListModel as "MapListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::MapListModel, _>(&ty)?;
        ty.implements::<MapListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::MapListModel>()?;
    }
);

pub type MediaControls = crate::newtype::Gtk<gtk4::MediaControls>;

crate::common::interface!(MediaControls as gtk4::MediaControls | _ty | {});

crate::common::class_def!(
    MediaControls as "MediaControls" |ty| {
        crate::core::from_gtype::define::<gtk4::MediaControls, _>(&ty)?;
        ty.implements::<MediaControls>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::MediaControls>()?;
    }
);

pub type MediaFile = crate::newtype::Gtk<gtk4::MediaFile>;

crate::common::interface!(MediaFile as gtk4::MediaFile | _ty | {});

crate::common::class_def!(
    MediaFile as "MediaFile" |ty| {
        crate::core::from_gtype::define::<gtk4::MediaFile, _>(&ty)?;
        ty.implements::<MediaFile>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::MediaStream>>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gtk4::MediaFile>()?;
    }
);

pub type MediaStream = crate::newtype::Gtk<gtk4::MediaStream>;

crate::common::interface!(MediaStream as gtk4::MediaStream | _ty | {});

crate::common::class_def!(
    MediaStream as "MediaStream" |ty| {
        crate::core::from_gtype::define::<gtk4::MediaStream, _>(&ty)?;
        ty.implements::<MediaStream>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gtk4::MediaStream>()?;
    }
);

pub type MenuButton = crate::newtype::Gtk<gtk4::MenuButton>;

crate::common::interface!(MenuButton as gtk4::MenuButton | _ty | {});

crate::common::class_def!(
    MenuButton as "MenuButton" |ty| {
        crate::core::from_gtype::define::<gtk4::MenuButton, _>(&ty)?;
        ty.implements::<MenuButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::MenuButton>()?;
    }
);

pub type MessageDialog = crate::newtype::Gtk<gtk4::MessageDialog>;

crate::common::interface!(MessageDialog as gtk4::MessageDialog | _ty | {});

crate::common::class_def!(
    MessageDialog as "MessageDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::MessageDialog, _>(&ty)?;
        ty.implements::<MessageDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::MessageDialog>()?;
    }
);

pub type MnemonicAction = crate::newtype::Gtk<gtk4::MnemonicAction>;

crate::common::interface!(MnemonicAction as gtk4::MnemonicAction | _ty | {});

crate::common::class_def!(
    MnemonicAction as "MnemonicAction" |ty| {
        crate::core::from_gtype::define::<gtk4::MnemonicAction, _>(&ty)?;
        ty.implements::<MnemonicAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::MnemonicAction>()?;
    }
);

pub type MnemonicTrigger = crate::newtype::Gtk<gtk4::MnemonicTrigger>;

crate::common::interface!(MnemonicTrigger as gtk4::MnemonicTrigger | _ty | {});

crate::common::class_def!(
    MnemonicTrigger as "MnemonicTrigger" |ty| {
        crate::core::from_gtype::define::<gtk4::MnemonicTrigger, _>(&ty)?;
        ty.implements::<MnemonicTrigger>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutTrigger>>()?;
        ty.register_as::<gtk4::MnemonicTrigger>()?;
    }
);

pub type MountOperation = crate::newtype::Gtk<gtk4::MountOperation>;

crate::common::interface!(MountOperation as gtk4::MountOperation | _ty | {});

crate::common::class_def!(
    MountOperation as "MountOperation" |ty| {
        crate::core::from_gtype::define::<gtk4::MountOperation, _>(&ty)?;
        ty.implements::<MountOperation>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::MountOperation>>()?;
        ty.register_as::<gtk4::MountOperation>()?;
    }
);

pub type MultiFilter = crate::newtype::Gtk<gtk4::MultiFilter>;

crate::common::interface!(MultiFilter as gtk4::MultiFilter | _ty | {});

crate::common::class_def!(
    MultiFilter as "MultiFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::MultiFilter, _>(&ty)?;
        ty.implements::<MultiFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::MultiFilter>()?;
    }
);

pub type MultiSelection = crate::newtype::Gtk<gtk4::MultiSelection>;

crate::common::interface!(MultiSelection as gtk4::MultiSelection | _ty | {});

crate::common::class_def!(
    MultiSelection as "MultiSelection" |ty| {
        crate::core::from_gtype::define::<gtk4::MultiSelection, _>(&ty)?;
        ty.implements::<MultiSelection>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::SelectionModel>>()?;
        ty.register_as::<gtk4::MultiSelection>()?;
    }
);

pub type MultiSorter = crate::newtype::Gtk<gtk4::MultiSorter>;

crate::common::interface!(MultiSorter as gtk4::MultiSorter | _ty | {});

crate::common::class_def!(
    MultiSorter as "MultiSorter" |ty| {
        crate::core::from_gtype::define::<gtk4::MultiSorter, _>(&ty)?;
        ty.implements::<MultiSorter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Sorter>>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::MultiSorter>()?;
    }
);

pub type NamedAction = crate::newtype::Gtk<gtk4::NamedAction>;

crate::common::interface!(NamedAction as gtk4::NamedAction | _ty | {});

crate::common::class_def!(
    NamedAction as "NamedAction" |ty| {
        crate::core::from_gtype::define::<gtk4::NamedAction, _>(&ty)?;
        ty.implements::<NamedAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::NamedAction>()?;
    }
);

pub type NativeDialog = crate::newtype::Gtk<gtk4::NativeDialog>;

crate::common::interface!(NativeDialog as gtk4::NativeDialog | _ty | {});

crate::common::class_def!(
    NativeDialog as "NativeDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::NativeDialog, _>(&ty)?;
        ty.implements::<NativeDialog>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::NativeDialog>()?;
    }
);

pub type NeverTrigger = crate::newtype::Gtk<gtk4::NeverTrigger>;

crate::common::interface!(NeverTrigger as gtk4::NeverTrigger | _ty | {});

crate::common::class_def!(
    NeverTrigger as "NeverTrigger" |ty| {
        crate::core::from_gtype::define::<gtk4::NeverTrigger, _>(&ty)?;
        ty.implements::<NeverTrigger>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutTrigger>>()?;
        ty.register_as::<gtk4::NeverTrigger>()?;
    }
);

pub type NoSelection = crate::newtype::Gtk<gtk4::NoSelection>;

crate::common::interface!(NoSelection as gtk4::NoSelection | _ty | {});

crate::common::class_def!(
    NoSelection as "NoSelection" |ty| {
        crate::core::from_gtype::define::<gtk4::NoSelection, _>(&ty)?;
        ty.implements::<NoSelection>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::SelectionModel>>()?;
        ty.register_as::<gtk4::NoSelection>()?;
    }
);

pub type Notebook = crate::newtype::Gtk<gtk4::Notebook>;

crate::common::interface!(Notebook as gtk4::Notebook | _ty | {});

crate::common::class_def!(
    Notebook as "Notebook" |ty| {
        crate::core::from_gtype::define::<gtk4::Notebook, _>(&ty)?;
        ty.implements::<Notebook>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Notebook>()?;
    }
);

pub type NotebookPage = crate::newtype::Gtk<gtk4::NotebookPage>;

crate::common::interface!(NotebookPage as gtk4::NotebookPage | _ty | {});

crate::common::class_def!(
    NotebookPage as "NotebookPage" |ty| {
        crate::core::from_gtype::define::<gtk4::NotebookPage, _>(&ty)?;
        ty.implements::<NotebookPage>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::NotebookPage>()?;
    }
);

pub type NothingAction = crate::newtype::Gtk<gtk4::NothingAction>;

crate::common::interface!(NothingAction as gtk4::NothingAction | _ty | {});

crate::common::class_def!(
    NothingAction as "NothingAction" |ty| {
        crate::core::from_gtype::define::<gtk4::NothingAction, _>(&ty)?;
        ty.implements::<NothingAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::NothingAction>()?;
    }
);

pub type NumericSorter = crate::newtype::Gtk<gtk4::NumericSorter>;

crate::common::interface!(NumericSorter as gtk4::NumericSorter | _ty | {});

crate::common::class_def!(
    NumericSorter as "NumericSorter" |ty| {
        crate::core::from_gtype::define::<gtk4::NumericSorter, _>(&ty)?;
        ty.implements::<NumericSorter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Sorter>>()?;
        ty.register_as::<gtk4::NumericSorter>()?;
    }
);

pub type Overlay = crate::newtype::Gtk<gtk4::Overlay>;

crate::common::interface!(Overlay as gtk4::Overlay | _ty | {});

crate::common::class_def!(
    Overlay as "Overlay" |ty| {
        crate::core::from_gtype::define::<gtk4::Overlay, _>(&ty)?;
        ty.implements::<Overlay>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Overlay>()?;
    }
);

pub type OverlayLayout = crate::newtype::Gtk<gtk4::OverlayLayout>;

crate::common::interface!(OverlayLayout as gtk4::OverlayLayout | _ty | {});

crate::common::class_def!(
    OverlayLayout as "OverlayLayout" |ty| {
        crate::core::from_gtype::define::<gtk4::OverlayLayout, _>(&ty)?;
        ty.implements::<OverlayLayout>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutManager>>()?;
        ty.register_as::<gtk4::OverlayLayout>()?;
    }
);

pub type OverlayLayoutChild = crate::newtype::Gtk<gtk4::OverlayLayoutChild>;

crate::common::interface!(OverlayLayoutChild as gtk4::OverlayLayoutChild | _ty | {});

crate::common::class_def!(
    OverlayLayoutChild as "OverlayLayoutChild" |ty| {
        crate::core::from_gtype::define::<gtk4::OverlayLayoutChild, _>(&ty)?;
        ty.implements::<OverlayLayoutChild>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::LayoutChild>>()?;
        ty.register_as::<gtk4::OverlayLayoutChild>()?;
    }
);

pub type PadController = crate::newtype::Gtk<gtk4::PadController>;

crate::common::interface!(PadController as gtk4::PadController | _ty | {});

crate::common::class_def!(
    PadController as "PadController" |ty| {
        crate::core::from_gtype::define::<gtk4::PadController, _>(&ty)?;
        ty.implements::<PadController>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.register_as::<gtk4::PadController>()?;
    }
);

pub type PageSetup = crate::newtype::Gtk<gtk4::PageSetup>;

crate::common::interface!(PageSetup as gtk4::PageSetup | _ty | {});

crate::common::class_def!(
    PageSetup as "PageSetup" |ty| {
        crate::core::from_gtype::define::<gtk4::PageSetup, _>(&ty)?;
        ty.implements::<PageSetup>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::PageSetup>()?;
    }
);

pub type PageSetupUnixDialog = crate::newtype::Gtk<gtk4::PageSetupUnixDialog>;

crate::common::interface!(PageSetupUnixDialog as gtk4::PageSetupUnixDialog | _ty | {});

crate::common::class_def!(
    PageSetupUnixDialog as "PageSetupUnixDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::PageSetupUnixDialog, _>(&ty)?;
        ty.implements::<PageSetupUnixDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::PageSetupUnixDialog>()?;
    }
);

pub type Paned = crate::newtype::Gtk<gtk4::Paned>;

crate::common::interface!(Paned as gtk4::Paned | _ty | {});

crate::common::class_def!(
    Paned as "Paned" |ty| {
        crate::core::from_gtype::define::<gtk4::Paned, _>(&ty)?;
        ty.implements::<Paned>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Paned>()?;
    }
);

pub type PasswordEntry = crate::newtype::Gtk<gtk4::PasswordEntry>;

crate::common::interface!(PasswordEntry as gtk4::PasswordEntry | _ty | {});

crate::common::class_def!(
    PasswordEntry as "PasswordEntry" |ty| {
        crate::core::from_gtype::define::<gtk4::PasswordEntry, _>(&ty)?;
        ty.implements::<PasswordEntry>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.register_as::<gtk4::PasswordEntry>()?;
    }
);

pub type Picture = crate::newtype::Gtk<gtk4::Picture>;

crate::common::interface!(Picture as gtk4::Picture | _ty | {});

crate::common::class_def!(
    Picture as "Picture" |ty| {
        crate::core::from_gtype::define::<gtk4::Picture, _>(&ty)?;
        ty.implements::<Picture>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Picture>()?;
    }
);

pub type Popover = crate::newtype::Gtk<gtk4::Popover>;

crate::common::interface!(Popover as gtk4::Popover | _ty | {});

crate::common::class_def!(
    Popover as "Popover" |ty| {
        crate::core::from_gtype::define::<gtk4::Popover, _>(&ty)?;
        ty.implements::<Popover>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::Popover>()?;
    }
);

pub type PopoverMenu = crate::newtype::Gtk<gtk4::PopoverMenu>;

crate::common::interface!(PopoverMenu as gtk4::PopoverMenu | _ty | {});

crate::common::class_def!(
    PopoverMenu as "PopoverMenu" |ty| {
        crate::core::from_gtype::define::<gtk4::PopoverMenu, _>(&ty)?;
        ty.implements::<PopoverMenu>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Popover>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::PopoverMenu>()?;
    }
);

pub type PopoverMenuBar = crate::newtype::Gtk<gtk4::PopoverMenuBar>;

crate::common::interface!(PopoverMenuBar as gtk4::PopoverMenuBar | _ty | {});

crate::common::class_def!(
    PopoverMenuBar as "PopoverMenuBar" |ty| {
        crate::core::from_gtype::define::<gtk4::PopoverMenuBar, _>(&ty)?;
        ty.implements::<PopoverMenuBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::PopoverMenuBar>()?;
    }
);

pub type PrintContext = crate::newtype::Gtk<gtk4::PrintContext>;

crate::common::interface!(PrintContext as gtk4::PrintContext | _ty | {});

crate::common::class_def!(
    PrintContext as "PrintContext" |ty| {
        crate::core::from_gtype::define::<gtk4::PrintContext, _>(&ty)?;
        ty.implements::<PrintContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::PrintContext>()?;
    }
);

pub type PrintJob = crate::newtype::Gtk<gtk4::PrintJob>;

crate::common::interface!(PrintJob as gtk4::PrintJob | _ty | {});

crate::common::class_def!(
    PrintJob as "PrintJob" |ty| {
        crate::core::from_gtype::define::<gtk4::PrintJob, _>(&ty)?;
        ty.implements::<PrintJob>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::PrintJob>()?;
    }
);

pub type PrintOperation = crate::newtype::Gtk<gtk4::PrintOperation>;

crate::common::interface!(PrintOperation as gtk4::PrintOperation | _ty | {});

crate::common::class_def!(
    PrintOperation as "PrintOperation" |ty| {
        crate::core::from_gtype::define::<gtk4::PrintOperation, _>(&ty)?;
        ty.implements::<PrintOperation>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::PrintOperationPreview>>()?;
        ty.register_as::<gtk4::PrintOperation>()?;
    }
);

pub type PrintSettings = crate::newtype::Gtk<gtk4::PrintSettings>;

crate::common::interface!(PrintSettings as gtk4::PrintSettings | _ty | {});

crate::common::class_def!(
    PrintSettings as "PrintSettings" |ty| {
        crate::core::from_gtype::define::<gtk4::PrintSettings, _>(&ty)?;
        ty.implements::<PrintSettings>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::PrintSettings>()?;
    }
);

pub type PrintUnixDialog = crate::newtype::Gtk<gtk4::PrintUnixDialog>;

crate::common::interface!(PrintUnixDialog as gtk4::PrintUnixDialog | _ty | {});

crate::common::class_def!(
    PrintUnixDialog as "PrintUnixDialog" |ty| {
        crate::core::from_gtype::define::<gtk4::PrintUnixDialog, _>(&ty)?;
        ty.implements::<PrintUnixDialog>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Dialog>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::PrintUnixDialog>()?;
    }
);

pub type Printer = crate::newtype::Gtk<gtk4::Printer>;

crate::common::interface!(Printer as gtk4::Printer | _ty | {});

crate::common::class_def!(
    Printer as "Printer" |ty| {
        crate::core::from_gtype::define::<gtk4::Printer, _>(&ty)?;
        ty.implements::<Printer>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Printer>()?;
    }
);

pub type ProgressBar = crate::newtype::Gtk<gtk4::ProgressBar>;

crate::common::interface!(ProgressBar as gtk4::ProgressBar | _ty | {});

crate::common::class_def!(
    ProgressBar as "ProgressBar" |ty| {
        crate::core::from_gtype::define::<gtk4::ProgressBar, _>(&ty)?;
        ty.implements::<ProgressBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::ProgressBar>()?;
    }
);

pub type Range = crate::newtype::Gtk<gtk4::Range>;

crate::common::interface!(Range as gtk4::Range | _ty | {});

crate::common::class_def!(
    Range as "Range" |ty| {
        crate::core::from_gtype::define::<gtk4::Range, _>(&ty)?;
        ty.implements::<Range>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Range>()?;
    }
);

pub type RecentManager = crate::newtype::Gtk<gtk4::RecentManager>;

crate::common::interface!(RecentManager as gtk4::RecentManager | _ty | {});

crate::common::class_def!(
    RecentManager as "RecentManager" |ty| {
        crate::core::from_gtype::define::<gtk4::RecentManager, _>(&ty)?;
        ty.implements::<RecentManager>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::RecentManager>()?;
    }
);

pub type Revealer = crate::newtype::Gtk<gtk4::Revealer>;

crate::common::interface!(Revealer as gtk4::Revealer | _ty | {});

crate::common::class_def!(
    Revealer as "Revealer" |ty| {
        crate::core::from_gtype::define::<gtk4::Revealer, _>(&ty)?;
        ty.implements::<Revealer>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Revealer>()?;
    }
);

pub type Scale = crate::newtype::Gtk<gtk4::Scale>;

crate::common::interface!(Scale as gtk4::Scale | _ty | {});

crate::common::class_def!(
    Scale as "Scale" |ty| {
        crate::core::from_gtype::define::<gtk4::Scale, _>(&ty)?;
        ty.implements::<Scale>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Range>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Scale>()?;
    }
);

pub type ScaleButton = crate::newtype::Gtk<gtk4::ScaleButton>;

crate::common::interface!(ScaleButton as gtk4::ScaleButton | _ty | {});

crate::common::class_def!(
    ScaleButton as "ScaleButton" |ty| {
        crate::core::from_gtype::define::<gtk4::ScaleButton, _>(&ty)?;
        ty.implements::<ScaleButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::ScaleButton>()?;
    }
);

pub type Scrollbar = crate::newtype::Gtk<gtk4::Scrollbar>;

crate::common::interface!(Scrollbar as gtk4::Scrollbar | _ty | {});

crate::common::class_def!(
    Scrollbar as "Scrollbar" |ty| {
        crate::core::from_gtype::define::<gtk4::Scrollbar, _>(&ty)?;
        ty.implements::<Scrollbar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Scrollbar>()?;
    }
);

pub type ScrolledWindow = crate::newtype::Gtk<gtk4::ScrolledWindow>;

crate::common::interface!(ScrolledWindow as gtk4::ScrolledWindow | _ty | {});

crate::common::class_def!(
    ScrolledWindow as "ScrolledWindow" |ty| {
        crate::core::from_gtype::define::<gtk4::ScrolledWindow, _>(&ty)?;
        ty.implements::<ScrolledWindow>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ScrolledWindow>()?;
    }
);

pub type SearchBar = crate::newtype::Gtk<gtk4::SearchBar>;

crate::common::interface!(SearchBar as gtk4::SearchBar | _ty | {});

crate::common::class_def!(
    SearchBar as "SearchBar" |ty| {
        crate::core::from_gtype::define::<gtk4::SearchBar, _>(&ty)?;
        ty.implements::<SearchBar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::SearchBar>()?;
    }
);

pub type SearchEntry = crate::newtype::Gtk<gtk4::SearchEntry>;

crate::common::interface!(SearchEntry as gtk4::SearchEntry | _ty | {});

crate::common::class_def!(
    SearchEntry as "SearchEntry" |ty| {
        crate::core::from_gtype::define::<gtk4::SearchEntry, _>(&ty)?;
        ty.implements::<SearchEntry>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.register_as::<gtk4::SearchEntry>()?;
    }
);

pub type SelectionFilterModel = crate::newtype::Gtk<gtk4::SelectionFilterModel>;

crate::common::interface!(SelectionFilterModel as gtk4::SelectionFilterModel | _ty | {});

crate::common::class_def!(
    SelectionFilterModel as "SelectionFilterModel" |ty| {
        crate::core::from_gtype::define::<gtk4::SelectionFilterModel, _>(&ty)?;
        ty.implements::<SelectionFilterModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::SelectionFilterModel>()?;
    }
);

pub type Separator = crate::newtype::Gtk<gtk4::Separator>;

crate::common::interface!(Separator as gtk4::Separator | _ty | {});

crate::common::class_def!(
    Separator as "Separator" |ty| {
        crate::core::from_gtype::define::<gtk4::Separator, _>(&ty)?;
        ty.implements::<Separator>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::Separator>()?;
    }
);

pub type Settings = crate::newtype::Gtk<gtk4::Settings>;

crate::common::interface!(Settings as gtk4::Settings | _ty | {});

crate::common::class_def!(
    Settings as "Settings" |ty| {
        crate::core::from_gtype::define::<gtk4::Settings, _>(&ty)?;
        ty.implements::<Settings>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::StyleProvider>>()?;
        ty.register_as::<gtk4::Settings>()?;
    }
);

pub type Shortcut = crate::newtype::Gtk<gtk4::Shortcut>;

crate::common::interface!(Shortcut as gtk4::Shortcut | _ty | {});

crate::common::class_def!(
    Shortcut as "Shortcut" |ty| {
        crate::core::from_gtype::define::<gtk4::Shortcut, _>(&ty)?;
        ty.implements::<Shortcut>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Shortcut>()?;
    }
);

pub type ShortcutAction = crate::newtype::Gtk<gtk4::ShortcutAction>;

crate::common::interface!(ShortcutAction as gtk4::ShortcutAction | _ty | {});

crate::common::class_def!(
    ShortcutAction as "ShortcutAction" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutAction, _>(&ty)?;
        ty.implements::<ShortcutAction>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ShortcutAction>()?;
    }
);

pub type ShortcutController = crate::newtype::Gtk<gtk4::ShortcutController>;

crate::common::interface!(ShortcutController as gtk4::ShortcutController | _ty | {});

crate::common::class_def!(
    ShortcutController as "ShortcutController" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutController, _>(&ty)?;
        ty.implements::<ShortcutController>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::EventController>>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::ShortcutController>()?;
    }
);

pub type ShortcutLabel = crate::newtype::Gtk<gtk4::ShortcutLabel>;

crate::common::interface!(ShortcutLabel as gtk4::ShortcutLabel | _ty | {});

crate::common::class_def!(
    ShortcutLabel as "ShortcutLabel" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutLabel, _>(&ty)?;
        ty.implements::<ShortcutLabel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ShortcutLabel>()?;
    }
);

pub type ShortcutTrigger = crate::newtype::Gtk<gtk4::ShortcutTrigger>;

crate::common::interface!(ShortcutTrigger as gtk4::ShortcutTrigger | _ty | {});

crate::common::class_def!(
    ShortcutTrigger as "ShortcutTrigger" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutTrigger, _>(&ty)?;
        ty.implements::<ShortcutTrigger>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::ShortcutTrigger>()?;
    }
);

pub type ShortcutsGroup = crate::newtype::Gtk<gtk4::ShortcutsGroup>;

crate::common::interface!(ShortcutsGroup as gtk4::ShortcutsGroup | _ty | {});

crate::common::class_def!(
    ShortcutsGroup as "ShortcutsGroup" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutsGroup, _>(&ty)?;
        ty.implements::<ShortcutsGroup>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Box>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::ShortcutsGroup>()?;
    }
);

pub type ShortcutsSection = crate::newtype::Gtk<gtk4::ShortcutsSection>;

crate::common::interface!(ShortcutsSection as gtk4::ShortcutsSection | _ty | {});

crate::common::class_def!(
    ShortcutsSection as "ShortcutsSection" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutsSection, _>(&ty)?;
        ty.implements::<ShortcutsSection>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Box>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::ShortcutsSection>()?;
    }
);

pub type ShortcutsShortcut = crate::newtype::Gtk<gtk4::ShortcutsShortcut>;

crate::common::interface!(ShortcutsShortcut as gtk4::ShortcutsShortcut | _ty | {});

crate::common::class_def!(
    ShortcutsShortcut as "ShortcutsShortcut" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutsShortcut, _>(&ty)?;
        ty.implements::<ShortcutsShortcut>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::ShortcutsShortcut>()?;
    }
);

pub type ShortcutsWindow = crate::newtype::Gtk<gtk4::ShortcutsWindow>;

crate::common::interface!(ShortcutsWindow as gtk4::ShortcutsWindow | _ty | {});

crate::common::class_def!(
    ShortcutsWindow as "ShortcutsWindow" |ty| {
        crate::core::from_gtype::define::<gtk4::ShortcutsWindow, _>(&ty)?;
        ty.implements::<ShortcutsWindow>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Window>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Native>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Root>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutManager>>()?;
        ty.register_as::<gtk4::ShortcutsWindow>()?;
    }
);

pub type SignalAction = crate::newtype::Gtk<gtk4::SignalAction>;

crate::common::interface!(SignalAction as gtk4::SignalAction | _ty | {});

crate::common::class_def!(
    SignalAction as "SignalAction" |ty| {
        crate::core::from_gtype::define::<gtk4::SignalAction, _>(&ty)?;
        ty.implements::<SignalAction>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ShortcutAction>>()?;
        ty.register_as::<gtk4::SignalAction>()?;
    }
);

pub type SignalListItemFactory = crate::newtype::Gtk<gtk4::SignalListItemFactory>;

crate::common::interface!(SignalListItemFactory as gtk4::SignalListItemFactory | _ty | {});

crate::common::class_def!(
    SignalListItemFactory as "SignalListItemFactory" |ty| {
        crate::core::from_gtype::define::<gtk4::SignalListItemFactory, _>(&ty)?;
        ty.implements::<SignalListItemFactory>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ListItemFactory>>()?;
        ty.register_as::<gtk4::SignalListItemFactory>()?;
    }
);

pub type SingleSelection = crate::newtype::Gtk<gtk4::SingleSelection>;

crate::common::interface!(SingleSelection as gtk4::SingleSelection | _ty | {});

crate::common::class_def!(
    SingleSelection as "SingleSelection" |ty| {
        crate::core::from_gtype::define::<gtk4::SingleSelection, _>(&ty)?;
        ty.implements::<SingleSelection>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::SelectionModel>>()?;
        ty.register_as::<gtk4::SingleSelection>()?;
    }
);

pub type SizeGroup = crate::newtype::Gtk<gtk4::SizeGroup>;

crate::common::interface!(SizeGroup as gtk4::SizeGroup | _ty | {});

crate::common::class_def!(
    SizeGroup as "SizeGroup" |ty| {
        crate::core::from_gtype::define::<gtk4::SizeGroup, _>(&ty)?;
        ty.implements::<SizeGroup>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::SizeGroup>()?;
    }
);

pub type SliceListModel = crate::newtype::Gtk<gtk4::SliceListModel>;

crate::common::interface!(SliceListModel as gtk4::SliceListModel | _ty | {});

crate::common::class_def!(
    SliceListModel as "SliceListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::SliceListModel, _>(&ty)?;
        ty.implements::<SliceListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::SliceListModel>()?;
    }
);

pub type SortListModel = crate::newtype::Gtk<gtk4::SortListModel>;

crate::common::interface!(SortListModel as gtk4::SortListModel | _ty | {});

crate::common::class_def!(
    SortListModel as "SortListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::SortListModel, _>(&ty)?;
        ty.implements::<SortListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::SortListModel>()?;
    }
);

pub type Sorter = crate::newtype::Gtk<gtk4::Sorter>;

crate::common::interface!(Sorter as gtk4::Sorter | _ty | {});

crate::common::class_def!(
    Sorter as "Sorter" |ty| {
        crate::core::from_gtype::define::<gtk4::Sorter, _>(&ty)?;
        ty.implements::<Sorter>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Sorter>()?;
    }
);

pub type SpinButton = crate::newtype::Gtk<gtk4::SpinButton>;

crate::common::interface!(SpinButton as gtk4::SpinButton | _ty | {});

crate::common::class_def!(
    SpinButton as "SpinButton" |ty| {
        crate::core::from_gtype::define::<gtk4::SpinButton, _>(&ty)?;
        ty.implements::<SpinButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellEditable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::SpinButton>()?;
    }
);

pub type Spinner = crate::newtype::Gtk<gtk4::Spinner>;

crate::common::interface!(Spinner as gtk4::Spinner | _ty | {});

crate::common::class_def!(
    Spinner as "Spinner" |ty| {
        crate::core::from_gtype::define::<gtk4::Spinner, _>(&ty)?;
        ty.implements::<Spinner>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Spinner>()?;
    }
);

pub type Stack = crate::newtype::Gtk<gtk4::Stack>;

crate::common::interface!(Stack as gtk4::Stack | _ty | {});

crate::common::class_def!(
    Stack as "Stack" |ty| {
        crate::core::from_gtype::define::<gtk4::Stack, _>(&ty)?;
        ty.implements::<Stack>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Stack>()?;
    }
);

pub type StackPage = crate::newtype::Gtk<gtk4::StackPage>;

crate::common::interface!(StackPage as gtk4::StackPage | _ty | {});

crate::common::class_def!(
    StackPage as "StackPage" |ty| {
        crate::core::from_gtype::define::<gtk4::StackPage, _>(&ty)?;
        ty.implements::<StackPage>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.register_as::<gtk4::StackPage>()?;
    }
);

pub type StackSidebar = crate::newtype::Gtk<gtk4::StackSidebar>;

crate::common::interface!(StackSidebar as gtk4::StackSidebar | _ty | {});

crate::common::class_def!(
    StackSidebar as "StackSidebar" |ty| {
        crate::core::from_gtype::define::<gtk4::StackSidebar, _>(&ty)?;
        ty.implements::<StackSidebar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::StackSidebar>()?;
    }
);

pub type StackSwitcher = crate::newtype::Gtk<gtk4::StackSwitcher>;

crate::common::interface!(StackSwitcher as gtk4::StackSwitcher | _ty | {});

crate::common::class_def!(
    StackSwitcher as "StackSwitcher" |ty| {
        crate::core::from_gtype::define::<gtk4::StackSwitcher, _>(&ty)?;
        ty.implements::<StackSwitcher>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::StackSwitcher>()?;
    }
);

pub type Statusbar = crate::newtype::Gtk<gtk4::Statusbar>;

crate::common::interface!(Statusbar as gtk4::Statusbar | _ty | {});

crate::common::class_def!(
    Statusbar as "Statusbar" |ty| {
        crate::core::from_gtype::define::<gtk4::Statusbar, _>(&ty)?;
        ty.implements::<Statusbar>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Statusbar>()?;
    }
);

pub type StringFilter = crate::newtype::Gtk<gtk4::StringFilter>;

crate::common::interface!(StringFilter as gtk4::StringFilter | _ty | {});

crate::common::class_def!(
    StringFilter as "StringFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::StringFilter, _>(&ty)?;
        ty.implements::<StringFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Filter>>()?;
        ty.register_as::<gtk4::StringFilter>()?;
    }
);

pub type StringList = crate::newtype::Gtk<gtk4::StringList>;

crate::common::interface!(StringList as gtk4::StringList | _ty | {});

crate::common::class_def!(
    StringList as "StringList" |ty| {
        crate::core::from_gtype::define::<gtk4::StringList, _>(&ty)?;
        ty.implements::<StringList>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::StringList>()?;
    }
);

pub type StringObject = crate::newtype::Gtk<gtk4::StringObject>;

crate::common::interface!(StringObject as gtk4::StringObject | _ty | {});

crate::common::class_def!(
    StringObject as "StringObject" |ty| {
        crate::core::from_gtype::define::<gtk4::StringObject, _>(&ty)?;
        ty.implements::<StringObject>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::StringObject>()?;
    }
);

pub type StringSorter = crate::newtype::Gtk<gtk4::StringSorter>;

crate::common::interface!(StringSorter as gtk4::StringSorter | _ty | {});

crate::common::class_def!(
    StringSorter as "StringSorter" |ty| {
        crate::core::from_gtype::define::<gtk4::StringSorter, _>(&ty)?;
        ty.implements::<StringSorter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Sorter>>()?;
        ty.register_as::<gtk4::StringSorter>()?;
    }
);

pub type StyleContext = crate::newtype::Gtk<gtk4::StyleContext>;

crate::common::interface!(StyleContext as gtk4::StyleContext | _ty | {});

crate::common::class_def!(
    StyleContext as "StyleContext" |ty| {
        crate::core::from_gtype::define::<gtk4::StyleContext, _>(&ty)?;
        ty.implements::<StyleContext>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::StyleContext>()?;
    }
);

pub type Switch = crate::newtype::Gtk<gtk4::Switch>;

crate::common::interface!(Switch as gtk4::Switch | _ty | {});

crate::common::class_def!(
    Switch as "Switch" |ty| {
        crate::core::from_gtype::define::<gtk4::Switch, _>(&ty)?;
        ty.implements::<Switch>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::Switch>()?;
    }
);

pub type Text = crate::newtype::Gtk<gtk4::Text>;

crate::common::interface!(Text as gtk4::Text | _ty | {});

crate::common::class_def!(
    Text as "Text" |ty| {
        crate::core::from_gtype::define::<gtk4::Text, _>(&ty)?;
        ty.implements::<Text>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Editable>>()?;
        ty.register_as::<gtk4::Text>()?;
    }
);

pub type TextBuffer = crate::newtype::Gtk<gtk4::TextBuffer>;

crate::common::interface!(TextBuffer as gtk4::TextBuffer | _ty | {});

crate::common::class_def!(
    TextBuffer as "TextBuffer" |ty| {
        crate::core::from_gtype::define::<gtk4::TextBuffer, _>(&ty)?;
        ty.implements::<TextBuffer>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TextBuffer>()?;
    }
);

pub type TextChildAnchor = crate::newtype::Gtk<gtk4::TextChildAnchor>;

crate::common::interface!(TextChildAnchor as gtk4::TextChildAnchor | _ty | {});

crate::common::class_def!(
    TextChildAnchor as "TextChildAnchor" |ty| {
        crate::core::from_gtype::define::<gtk4::TextChildAnchor, _>(&ty)?;
        ty.implements::<TextChildAnchor>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TextChildAnchor>()?;
    }
);

pub type TextMark = crate::newtype::Gtk<gtk4::TextMark>;

crate::common::interface!(TextMark as gtk4::TextMark | _ty | {});

crate::common::class_def!(
    TextMark as "TextMark" |ty| {
        crate::core::from_gtype::define::<gtk4::TextMark, _>(&ty)?;
        ty.implements::<TextMark>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TextMark>()?;
    }
);

pub type TextTag = crate::newtype::Gtk<gtk4::TextTag>;

crate::common::interface!(TextTag as gtk4::TextTag | _ty | {});

crate::common::class_def!(
    TextTag as "TextTag" |ty| {
        crate::core::from_gtype::define::<gtk4::TextTag, _>(&ty)?;
        ty.implements::<TextTag>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TextTag>()?;
    }
);

pub type TextTagTable = crate::newtype::Gtk<gtk4::TextTagTable>;

crate::common::interface!(TextTagTable as gtk4::TextTagTable | _ty | {});

crate::common::class_def!(
    TextTagTable as "TextTagTable" |ty| {
        crate::core::from_gtype::define::<gtk4::TextTagTable, _>(&ty)?;
        ty.implements::<TextTagTable>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.register_as::<gtk4::TextTagTable>()?;
    }
);

pub type TextView = crate::newtype::Gtk<gtk4::TextView>;

crate::common::interface!(TextView as gtk4::TextView | _ty | {});

crate::common::class_def!(
    TextView as "TextView" |ty| {
        crate::core::from_gtype::define::<gtk4::TextView, _>(&ty)?;
        ty.implements::<TextView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::TextView>()?;
    }
);

pub type ToggleButton = crate::newtype::Gtk<gtk4::ToggleButton>;

crate::common::interface!(ToggleButton as gtk4::ToggleButton | _ty | {});

crate::common::class_def!(
    ToggleButton as "ToggleButton" |ty| {
        crate::core::from_gtype::define::<gtk4::ToggleButton, _>(&ty)?;
        ty.implements::<ToggleButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Button>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Actionable>>()?;
        ty.register_as::<gtk4::ToggleButton>()?;
    }
);

pub type Tooltip = crate::newtype::Gtk<gtk4::Tooltip>;

crate::common::interface!(Tooltip as gtk4::Tooltip | _ty | {});

crate::common::class_def!(
    Tooltip as "Tooltip" |ty| {
        crate::core::from_gtype::define::<gtk4::Tooltip, _>(&ty)?;
        ty.implements::<Tooltip>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::Tooltip>()?;
    }
);

pub type TreeExpander = crate::newtype::Gtk<gtk4::TreeExpander>;

crate::common::interface!(TreeExpander as gtk4::TreeExpander | _ty | {});

crate::common::class_def!(
    TreeExpander as "TreeExpander" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeExpander, _>(&ty)?;
        ty.implements::<TreeExpander>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::TreeExpander>()?;
    }
);

pub type TreeListModel = crate::newtype::Gtk<gtk4::TreeListModel>;

crate::common::interface!(TreeListModel as gtk4::TreeListModel | _ty | {});

crate::common::class_def!(
    TreeListModel as "TreeListModel" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeListModel, _>(&ty)?;
        ty.implements::<TreeListModel>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gio::ListModel>>()?;
        ty.register_as::<gtk4::TreeListModel>()?;
    }
);

pub type TreeListRow = crate::newtype::Gtk<gtk4::TreeListRow>;

crate::common::interface!(TreeListRow as gtk4::TreeListRow | _ty | {});

crate::common::class_def!(
    TreeListRow as "TreeListRow" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeListRow, _>(&ty)?;
        ty.implements::<TreeListRow>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TreeListRow>()?;
    }
);

pub type TreeListRowSorter = crate::newtype::Gtk<gtk4::TreeListRowSorter>;

crate::common::interface!(TreeListRowSorter as gtk4::TreeListRowSorter | _ty | {});

crate::common::class_def!(
    TreeListRowSorter as "TreeListRowSorter" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeListRowSorter, _>(&ty)?;
        ty.implements::<TreeListRowSorter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Sorter>>()?;
        ty.register_as::<gtk4::TreeListRowSorter>()?;
    }
);

pub type TreeModelFilter = crate::newtype::Gtk<gtk4::TreeModelFilter>;

crate::common::interface!(TreeModelFilter as gtk4::TreeModelFilter | _ty | {});

crate::common::class_def!(
    TreeModelFilter as "TreeModelFilter" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeModelFilter, _>(&ty)?;
        ty.implements::<TreeModelFilter>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragSource>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeModel>>()?;
        ty.register_as::<gtk4::TreeModelFilter>()?;
    }
);

pub type TreeModelSort = crate::newtype::Gtk<gtk4::TreeModelSort>;

crate::common::interface!(TreeModelSort as gtk4::TreeModelSort | _ty | {});

crate::common::class_def!(
    TreeModelSort as "TreeModelSort" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeModelSort, _>(&ty)?;
        ty.implements::<TreeModelSort>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragSource>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeSortable>>()?;
        ty.register_as::<gtk4::TreeModelSort>()?;
    }
);

pub type TreeSelection = crate::newtype::Gtk<gtk4::TreeSelection>;

crate::common::interface!(TreeSelection as gtk4::TreeSelection | _ty | {});

crate::common::class_def!(
    TreeSelection as "TreeSelection" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeSelection, _>(&ty)?;
        ty.implements::<TreeSelection>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::TreeSelection>()?;
    }
);

pub type TreeStore = crate::newtype::Gtk<gtk4::TreeStore>;

crate::common::interface!(TreeStore as gtk4::TreeStore | _ty | {});

crate::common::class_def!(
    TreeStore as "TreeStore" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeStore, _>(&ty)?;
        ty.implements::<TreeStore>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragDest>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeDragSource>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeModel>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::TreeSortable>>()?;
        ty.register_as::<gtk4::TreeStore>()?;
    }
);

pub type TreeView = crate::newtype::Gtk<gtk4::TreeView>;

crate::common::interface!(TreeView as gtk4::TreeView | _ty | {});

crate::common::class_def!(
    TreeView as "TreeView" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeView, _>(&ty)?;
        ty.implements::<TreeView>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::TreeView>()?;
    }
);

pub type TreeViewColumn = crate::newtype::Gtk<gtk4::TreeViewColumn>;

crate::common::interface!(TreeViewColumn as gtk4::TreeViewColumn | _ty | {});

crate::common::class_def!(
    TreeViewColumn as "TreeViewColumn" |ty| {
        crate::core::from_gtype::define::<gtk4::TreeViewColumn, _>(&ty)?;
        ty.implements::<TreeViewColumn>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::CellLayout>>()?;
        ty.register_as::<gtk4::TreeViewColumn>()?;
    }
);

pub type Video = crate::newtype::Gtk<gtk4::Video>;

crate::common::interface!(Video as gtk4::Video | _ty | {});

crate::common::class_def!(
    Video as "Video" |ty| {
        crate::core::from_gtype::define::<gtk4::Video, _>(&ty)?;
        ty.implements::<Video>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::Video>()?;
    }
);

pub type Viewport = crate::newtype::Gtk<gtk4::Viewport>;

crate::common::interface!(Viewport as gtk4::Viewport | _ty | {});

crate::common::class_def!(
    Viewport as "Viewport" |ty| {
        crate::core::from_gtype::define::<gtk4::Viewport, _>(&ty)?;
        ty.implements::<Viewport>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Scrollable>>()?;
        ty.register_as::<gtk4::Viewport>()?;
    }
);

pub type VolumeButton = crate::newtype::Gtk<gtk4::VolumeButton>;

crate::common::interface!(VolumeButton as gtk4::VolumeButton | _ty | {});

crate::common::class_def!(
    VolumeButton as "VolumeButton" |ty| {
        crate::core::from_gtype::define::<gtk4::VolumeButton, _>(&ty)?;
        ty.implements::<VolumeButton>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ScaleButton>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Orientable>>()?;
        ty.register_as::<gtk4::VolumeButton>()?;
    }
);

pub type WidgetPaintable = crate::newtype::Gtk<gtk4::WidgetPaintable>;

crate::common::interface!(WidgetPaintable as gtk4::WidgetPaintable | _ty | {});

crate::common::class_def!(
    WidgetPaintable as "WidgetPaintable" |ty| {
        crate::core::from_gtype::define::<gtk4::WidgetPaintable, _>(&ty)?;
        ty.implements::<WidgetPaintable>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gdk4::Paintable>>()?;
        ty.register_as::<gtk4::WidgetPaintable>()?;
    }
);

pub type WindowControls = crate::newtype::Gtk<gtk4::WindowControls>;

crate::common::interface!(WindowControls as gtk4::WindowControls | _ty | {});

crate::common::class_def!(
    WindowControls as "WindowControls" |ty| {
        crate::core::from_gtype::define::<gtk4::WindowControls, _>(&ty)?;
        ty.implements::<WindowControls>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::WindowControls>()?;
    }
);

pub type WindowGroup = crate::newtype::Gtk<gtk4::WindowGroup>;

crate::common::interface!(WindowGroup as gtk4::WindowGroup | _ty | {});

crate::common::class_def!(
    WindowGroup as "WindowGroup" |ty| {
        crate::core::from_gtype::define::<gtk4::WindowGroup, _>(&ty)?;
        ty.implements::<WindowGroup>()?;
        ty.implements::<crate::Object>()?;

        ty.register_as::<gtk4::WindowGroup>()?;
    }
);

pub type WindowHandle = crate::newtype::Gtk<gtk4::WindowHandle>;

crate::common::interface!(WindowHandle as gtk4::WindowHandle | _ty | {});

crate::common::class_def!(
    WindowHandle as "WindowHandle" |ty| {
        crate::core::from_gtype::define::<gtk4::WindowHandle, _>(&ty)?;
        ty.implements::<WindowHandle>()?;
        ty.implements::<crate::Object>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Widget>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::glib::InitiallyUnowned>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Accessible>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::Buildable>>()?;
        ty.implements::<crate::newtype::Gtk<gtk4::ConstraintTarget>>()?;
        ty.register_as::<gtk4::WindowHandle>()?;
    }
);

pub fn initialize_objects<'js>(ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<()> {
    ctx.register_type::<crate::newtype::Gtk<gdk4::AppLaunchContext>>("AppLaunchContext")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::CairoContext>>("CairoContext")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Clipboard>>("Clipboard")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::ContentDeserializer>>("ContentDeserializer")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::ContentProvider>>("ContentProvider")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::ContentSerializer>>("ContentSerializer")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Cursor>>("Cursor")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Device>>("Device")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::DeviceTool>>("DeviceTool")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Display>>("Display")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::DisplayManager>>("DisplayManager")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Drag>>("Drag")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::DrawContext>>("DrawContext")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Drop>>("Drop")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::FrameClock>>("FrameClock")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::GLContext>>("GLContext")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::GLTexture>>("GLTexture")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::MemoryTexture>>("MemoryTexture")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Monitor>>("Monitor")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Seat>>("Seat")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Surface>>("Surface")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::Texture>>("Texture")?;
    ctx.register_type::<crate::newtype::Gtk<gdk4::VulkanContext>>("VulkanContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ATContext>>("ATContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AboutDialog>>("AboutDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ActionBar>>("ActionBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ActivateAction>>("ActivateAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Adjustment>>("Adjustment")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AlternativeTrigger>>("AlternativeTrigger")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AnyFilter>>("AnyFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AppChooserButton>>("AppChooserButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AppChooserDialog>>("AppChooserDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AppChooserWidget>>("AppChooserWidget")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AspectFrame>>("AspectFrame")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Assistant>>("Assistant")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::AssistantPage>>("AssistantPage")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::BinLayout>>("BinLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::BookmarkList>>("BookmarkList")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::BoolFilter>>("BoolFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::BoxLayout>>("BoxLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Builder>>("Builder")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::BuilderListItemFactory>>(
        "BuilderListItemFactory",
    )?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Calendar>>("Calendar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CallbackAction>>("CallbackAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellArea>>("CellArea")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellAreaBox>>("CellAreaBox")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellAreaContext>>("CellAreaContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRenderer>>("CellRenderer")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererAccel>>("CellRendererAccel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererCombo>>("CellRendererCombo")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererPixbuf>>("CellRendererPixbuf")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererProgress>>("CellRendererProgress")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererSpin>>("CellRendererSpin")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererSpinner>>("CellRendererSpinner")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererText>>("CellRendererText")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellRendererToggle>>("CellRendererToggle")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CellView>>("CellView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CenterBox>>("CenterBox")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CenterLayout>>("CenterLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CheckButton>>("CheckButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ColorButton>>("ColorButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ColorChooserDialog>>("ColorChooserDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ColorChooserWidget>>("ColorChooserWidget")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ColumnView>>("ColumnView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ColumnViewColumn>>("ColumnViewColumn")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ComboBox>>("ComboBox")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ComboBoxText>>("ComboBoxText")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Constraint>>("Constraint")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ConstraintGuide>>("ConstraintGuide")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ConstraintLayout>>("ConstraintLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ConstraintLayoutChild>>("ConstraintLayoutChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CustomFilter>>("CustomFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::CustomSorter>>("CustomSorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Dialog>>("Dialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DirectoryList>>("DirectoryList")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DragIcon>>("DragIcon")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DragSource>>("DragSource")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DrawingArea>>("DrawingArea")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DropControllerMotion>>("DropControllerMotion")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DropDown>>("DropDown")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DropTarget>>("DropTarget")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::DropTargetAsync>>("DropTargetAsync")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EditableLabel>>("EditableLabel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EmojiChooser>>("EmojiChooser")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Entry>>("Entry")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EntryBuffer>>("EntryBuffer")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EntryCompletion>>("EntryCompletion")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventController>>("EventController")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventControllerFocus>>("EventControllerFocus")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventControllerKey>>("EventControllerKey")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventControllerLegacy>>("EventControllerLegacy")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventControllerMotion>>("EventControllerMotion")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EventControllerScroll>>("EventControllerScroll")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::EveryFilter>>("EveryFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Expander>>("Expander")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FileChooserDialog>>("FileChooserDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FileChooserNative>>("FileChooserNative")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FileChooserWidget>>("FileChooserWidget")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FileFilter>>("FileFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Filter>>("Filter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FilterListModel>>("FilterListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Fixed>>("Fixed")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FixedLayout>>("FixedLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FixedLayoutChild>>("FixedLayoutChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FlattenListModel>>("FlattenListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FlowBox>>("FlowBox")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FlowBoxChild>>("FlowBoxChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FontButton>>("FontButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FontChooserDialog>>("FontChooserDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::FontChooserWidget>>("FontChooserWidget")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Frame>>("Frame")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GLArea>>("GLArea")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Gesture>>("Gesture")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureClick>>("GestureClick")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureDrag>>("GestureDrag")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureLongPress>>("GestureLongPress")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GesturePan>>("GesturePan")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureRotate>>("GestureRotate")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureSingle>>("GestureSingle")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureStylus>>("GestureStylus")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureSwipe>>("GestureSwipe")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GestureZoom>>("GestureZoom")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Grid>>("Grid")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GridLayout>>("GridLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GridLayoutChild>>("GridLayoutChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::GridView>>("GridView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::HeaderBar>>("HeaderBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IMContext>>("IMContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IMContextSimple>>("IMContextSimple")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IMMulticontext>>("IMMulticontext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IconPaintable>>("IconPaintable")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IconTheme>>("IconTheme")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::IconView>>("IconView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Image>>("Image")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::InfoBar>>("InfoBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::KeyvalTrigger>>("KeyvalTrigger")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Label>>("Label")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::LayoutChild>>("LayoutChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::LayoutManager>>("LayoutManager")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::LevelBar>>("LevelBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::LinkButton>>("LinkButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListBase>>("ListBase")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListBox>>("ListBox")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListBoxRow>>("ListBoxRow")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListItem>>("ListItem")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListItemFactory>>("ListItemFactory")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListStore>>("ListStore")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ListView>>("ListView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::LockButton>>("LockButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MapListModel>>("MapListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MediaControls>>("MediaControls")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MediaFile>>("MediaFile")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MediaStream>>("MediaStream")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MenuButton>>("MenuButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MessageDialog>>("MessageDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MnemonicAction>>("MnemonicAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MnemonicTrigger>>("MnemonicTrigger")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MountOperation>>("MountOperation")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MultiFilter>>("MultiFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MultiSelection>>("MultiSelection")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::MultiSorter>>("MultiSorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NamedAction>>("NamedAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NativeDialog>>("NativeDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NeverTrigger>>("NeverTrigger")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NoSelection>>("NoSelection")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Notebook>>("Notebook")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NotebookPage>>("NotebookPage")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NothingAction>>("NothingAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::NumericSorter>>("NumericSorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Overlay>>("Overlay")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::OverlayLayout>>("OverlayLayout")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::OverlayLayoutChild>>("OverlayLayoutChild")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PadController>>("PadController")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PageSetup>>("PageSetup")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PageSetupUnixDialog>>("PageSetupUnixDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Paned>>("Paned")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PasswordEntry>>("PasswordEntry")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Picture>>("Picture")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Popover>>("Popover")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PopoverMenu>>("PopoverMenu")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PopoverMenuBar>>("PopoverMenuBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PrintContext>>("PrintContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PrintJob>>("PrintJob")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PrintOperation>>("PrintOperation")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PrintSettings>>("PrintSettings")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::PrintUnixDialog>>("PrintUnixDialog")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Printer>>("Printer")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ProgressBar>>("ProgressBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Range>>("Range")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::RecentManager>>("RecentManager")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Revealer>>("Revealer")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Scale>>("Scale")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ScaleButton>>("ScaleButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Scrollbar>>("Scrollbar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ScrolledWindow>>("ScrolledWindow")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SearchBar>>("SearchBar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SearchEntry>>("SearchEntry")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SelectionFilterModel>>("SelectionFilterModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Separator>>("Separator")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Settings>>("Settings")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Shortcut>>("Shortcut")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutAction>>("ShortcutAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutController>>("ShortcutController")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutLabel>>("ShortcutLabel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutTrigger>>("ShortcutTrigger")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutsGroup>>("ShortcutsGroup")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutsSection>>("ShortcutsSection")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutsShortcut>>("ShortcutsShortcut")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ShortcutsWindow>>("ShortcutsWindow")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SignalAction>>("SignalAction")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SignalListItemFactory>>("SignalListItemFactory")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SingleSelection>>("SingleSelection")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SizeGroup>>("SizeGroup")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SliceListModel>>("SliceListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SortListModel>>("SortListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Sorter>>("Sorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::SpinButton>>("SpinButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Spinner>>("Spinner")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Stack>>("Stack")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StackPage>>("StackPage")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StackSidebar>>("StackSidebar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StackSwitcher>>("StackSwitcher")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Statusbar>>("Statusbar")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StringFilter>>("StringFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StringList>>("StringList")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StringObject>>("StringObject")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StringSorter>>("StringSorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::StyleContext>>("StyleContext")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Switch>>("Switch")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Text>>("Text")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextBuffer>>("TextBuffer")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextChildAnchor>>("TextChildAnchor")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextMark>>("TextMark")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextTag>>("TextTag")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextTagTable>>("TextTagTable")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TextView>>("TextView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::ToggleButton>>("ToggleButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Tooltip>>("Tooltip")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeExpander>>("TreeExpander")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeListModel>>("TreeListModel")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeListRow>>("TreeListRow")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeListRowSorter>>("TreeListRowSorter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeModelFilter>>("TreeModelFilter")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeModelSort>>("TreeModelSort")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeSelection>>("TreeSelection")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeStore>>("TreeStore")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeView>>("TreeView")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::TreeViewColumn>>("TreeViewColumn")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Video>>("Video")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::Viewport>>("Viewport")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::VolumeButton>>("VolumeButton")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::WidgetPaintable>>("WidgetPaintable")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::WindowControls>>("WindowControls")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::WindowGroup>>("WindowGroup")?;
    ctx.register_type::<crate::newtype::Gtk<gtk4::WindowHandle>>("WindowHandle")?;
    Ok(())
}
