use std::rc::Rc;

use glib::MainContext;
use gtk4::{prelude::*, Application, Widget};
use rquickjs::{
    BuiltinResolver, ClassDef, Context, Ctx, Error, Executor, ExecutorSpawner, FromJs, Function,
    IntoJs, Loader, ModuleLoader, Object, Resolver, Result, This, Value,
};
use smallbox::smallbox;
use static_assertions::assert_impl_all;
use svelters_core::{exception, prelude::*};

use crate::macros::props;
use crate::{application, core, inner::*, wip};

pub fn initialize_elements(mut builder: TypeBuilder) {
    builder.register_interface("Object", smallbox!(core::object_interface));
    builder.register_interface("Widget", smallbox!(core::widget_interface));
    builder.register_interface("Accessible", smallbox!(core::accessible_interface));
    builder.register_interface("Window", smallbox!(core::window_interface));
    builder.register_interface(
        "ApplicationWindow",
        smallbox!(core::application_window_interface),
    );

    builder.register_interface("Button", smallbox!(wip::button_interface));
    builder.register_interface("ActionBar", smallbox!(wip::action_bar_interface));
    builder.register_interface("Actionable", smallbox!(wip::actionable_interface));
    builder.register_interface(
        "AppChooserButton",
        smallbox!(wip::app_chooser_button_interface),
    );
    builder.register_interface(
        "AppChooserWidget",
        smallbox!(wip::app_chooser_widget_interface),
    );
    builder.register_interface("AppChooser", smallbox!(wip::app_chooser_interface));
    builder.register_interface("AspectFrame", smallbox!(wip::aspect_frame_interface));
    builder.register_interface("Box", smallbox!(wip::box_interface));
    builder.register_interface("Calendar", smallbox!(wip::calendar_interface));
    builder.register_interface("CellEditable", smallbox!(wip::cell_editable_interface));
    builder.register_interface("CellView", smallbox!(wip::cell_view_interface));
    builder.register_interface("CenterBox", smallbox!(wip::center_box_interface));
    builder.register_interface("CheckButton", smallbox!(wip::check_button_interface));
    builder.register_interface("ColorButton", smallbox!(wip::color_button_interface));
    builder.register_interface(
        "ColorChooserWidget",
        smallbox!(wip::color_chooser_widget_interface),
    );
    builder.register_interface("ColumnView", smallbox!(wip::column_view_interface));
    builder.register_interface("ComboBox", smallbox!(wip::combo_box_interface));
    builder.register_interface("DragIcon", smallbox!(wip::drag_icon_interface));
    builder.register_interface("DrawingArea", smallbox!(wip::drawing_area_interface));
    builder.register_interface("DropDown", smallbox!(wip::drop_down_interface));
    builder.register_interface("EditableLabel", smallbox!(wip::editable_label_interface));
    builder.register_interface("Editable", smallbox!(wip::editable_interface));
    builder.register_interface("Entry", smallbox!(wip::entry_interface));
    builder.register_interface("Expander", smallbox!(wip::expander_interface));
    builder.register_interface(
        "FileChooserWidget",
        smallbox!(wip::file_chooser_widget_interface),
    );
    builder.register_interface("Fixed", smallbox!(wip::fixed_interface));
    builder.register_interface("FlowBoxChild", smallbox!(wip::flow_box_child_interface));
    builder.register_interface("FlowBox", smallbox!(wip::flow_box_interface));
    builder.register_interface("FontButton", smallbox!(wip::font_button_interface));
    builder.register_interface(
        "FontChooserWidget",
        smallbox!(wip::font_chooser_widget_interface),
    );
    builder.register_interface("Frame", smallbox!(wip::frame_interface));
    builder.register_interface("GlArea", smallbox!(wip::gl_area_interface));
    builder.register_interface("Grid", smallbox!(wip::grid_interface));
    builder.register_interface("HeaderBar", smallbox!(wip::header_bar_interface));
    builder.register_interface("IconView", smallbox!(wip::icon_view_interface));
    builder.register_interface("Image", smallbox!(wip::image_interface));
    builder.register_interface("InfoBar", smallbox!(wip::info_bar_interface));
    builder.register_interface("Label", smallbox!(wip::label_interface));
    builder.register_interface("LevelBar", smallbox!(wip::level_bar_interface));
    builder.register_interface("ListBase", smallbox!(wip::list_base_interface));
    builder.register_interface("ListBoxRow", smallbox!(wip::list_box_row_interface));
    builder.register_interface("ListBox", smallbox!(wip::list_box_interface));
    builder.register_interface("MediaControls", smallbox!(wip::media_controls_interface));
    builder.register_interface("MenuButton", smallbox!(wip::menu_button_interface));
    builder.register_interface("Native", smallbox!(wip::native_interface));
    builder.register_interface("Notebook", smallbox!(wip::notebook_interface));
    builder.register_interface("Overlay", smallbox!(wip::overlay_interface));
    builder.register_interface("Paned", smallbox!(wip::paned_interface));
    builder.register_interface("PasswordEntry", smallbox!(wip::password_entry_interface));
    builder.register_interface("Picture", smallbox!(wip::picture_interface));
    builder.register_interface("PopoverMenuBar", smallbox!(wip::popover_menu_bar_interface));
    builder.register_interface("Popover", smallbox!(wip::popover_interface));
    builder.register_interface("ProgressBar", smallbox!(wip::progress_bar_interface));
    builder.register_interface("Range", smallbox!(wip::range_interface));
    builder.register_interface("Revealer", smallbox!(wip::revealer_interface));
    builder.register_interface("Root", smallbox!(wip::root_interface));
    builder.register_interface("ScaleButton", smallbox!(wip::scale_button_interface));
    builder.register_interface("Scrollbar", smallbox!(wip::scrollbar_interface));
    builder.register_interface("ScrolledWindow", smallbox!(wip::scrolled_window_interface));
    builder.register_interface("SearchBar", smallbox!(wip::search_bar_interface));
    builder.register_interface("SearchEntry", smallbox!(wip::search_entry_interface));
    builder.register_interface("Separator", smallbox!(wip::separator_interface));
    builder.register_interface("ShortcutLabel", smallbox!(wip::shortcut_label_interface));
    builder.register_interface(
        "ShortcutsShortcut",
        smallbox!(wip::shortcuts_shortcut_interface),
    );
    builder.register_interface("SpinButton", smallbox!(wip::spin_button_interface));
    builder.register_interface("Spinner", smallbox!(wip::spinner_interface));
    builder.register_interface("StackSidebar", smallbox!(wip::stack_sidebar_interface));
    builder.register_interface("StackSwitcher", smallbox!(wip::stack_switcher_interface));
    builder.register_interface("Stack", smallbox!(wip::stack_interface));
    builder.register_interface("Statusbar", smallbox!(wip::statusbar_interface));
    builder.register_interface("Switch", smallbox!(wip::switch_interface));
    builder.register_interface("TextView", smallbox!(wip::text_view_interface));
    builder.register_interface("Text", smallbox!(wip::text_interface));
    builder.register_interface("TreeExpander", smallbox!(wip::tree_expander_interface));
    builder.register_interface("TreeView", smallbox!(wip::tree_view_interface));
    builder.register_interface("Video", smallbox!(wip::video_interface));
    builder.register_interface("Viewport", smallbox!(wip::viewport_interface));
    builder.register_interface("WindowControls", smallbox!(wip::window_controls_interface));
    builder.register_interface("WindowHandle", smallbox!(wip::window_handle_interface));

    builder.register_element("button", Rc::new(wip::button));
    builder.register_element("actionBar", Rc::new(wip::action_bar));
    // builder.register_element("actionable", Rc::new(wip::actionable));
    // builder.register_element("appChooserButton", Rc::new(wip::app_chooser_button));
    // builder.register_element("appChooserWidget", Rc::new(wip::app_chooser_widget));
    // builder.register_element("appChooser", Rc::new(wip::app_chooser));
    // builder.register_element("aspectFrame", Rc::new(wip::aspect_frame));
    builder.register_element("vBox", Rc::new(wip::vbox));
    builder.register_element("hBox", Rc::new(wip::hbox));
    builder.register_element("calendar", Rc::new(wip::calendar));
    // builder.register_element("cellEditable", Rc::new(wip::cell_editable));
    builder.register_element("cellView", Rc::new(wip::cell_view));
    builder.register_element("centerBox", Rc::new(wip::center_box));
    builder.register_element("checkButton", Rc::new(wip::check_button));
    builder.register_element("colorButton", Rc::new(wip::color_button));
    builder.register_element("colorChooserWidget", Rc::new(wip::color_chooser_widget));
    builder.register_element("columnView", Rc::new(wip::column_view));
    builder.register_element("comboBox", Rc::new(wip::combo_box));
    // builder.register_element("dragIcon", Rc::new(wip::drag_icon));
    builder.register_element("drawingArea", Rc::new(wip::drawing_area));
    // builder.register_element("dropDown", Rc::new(wip::drop_down));
    builder.register_element("editableLabel", Rc::new(wip::editable_label));
    // builder.register_element("editable", Rc::new(wip::editable));
    builder.register_element("entry", Rc::new(wip::entry));
    builder.register_element("expander", Rc::new(wip::expander));
    // builder.register_element("fileChooserWidget", Rc::new(wip::file_chooser_widget));
    builder.register_element("fixed", Rc::new(wip::fixed));
    builder.register_element("flowBoxChild", Rc::new(wip::flow_box_child));
    builder.register_element("flowBox", Rc::new(wip::flow_box));
    builder.register_element("fontButton", Rc::new(wip::font_button));
    builder.register_element("fontChooserWidget", Rc::new(wip::font_chooser_widget));
    builder.register_element("frame", Rc::new(wip::frame));
    builder.register_element("glArea", Rc::new(wip::gl_area));
    builder.register_element("grid", Rc::new(wip::grid));
    builder.register_element("headerBar", Rc::new(wip::header_bar));
    builder.register_element("iconView", Rc::new(wip::icon_view));
    builder.register_element("image", Rc::new(wip::image));
    builder.register_element("infoBar", Rc::new(wip::info_bar));
    builder.register_element("label", Rc::new(wip::label));
    builder.register_element("levelBar", Rc::new(wip::level_bar));
    // builder.register_element("listBase", Rc::new(wip::list_base));
    builder.register_element("listBoxRow", Rc::new(wip::list_box_row));
    builder.register_element("listBox", Rc::new(wip::list_box));
    builder.register_element("mediaControls", Rc::new(wip::media_controls));
    builder.register_element("menuButton", Rc::new(wip::menu_button));
    // builder.register_element("native", Rc::new(wip::native));
    builder.register_element("notebook", Rc::new(wip::notebook));
    builder.register_element("overlay", Rc::new(wip::overlay));
    builder.register_element("vPaned", Rc::new(wip::vpaned));
    builder.register_element("hPaned", Rc::new(wip::hpaned));
    builder.register_element("passwordEntry", Rc::new(wip::password_entry));
    builder.register_element("picture", Rc::new(wip::picture));
    // builder.register_element("popoverMenuBar", Rc::new(wip::popover_menu_bar));
    builder.register_element("popover", Rc::new(wip::popover));
    builder.register_element("progressBar", Rc::new(wip::progress_bar));
    // builder.register_element("range", Rc::new(wip::range));
    builder.register_element("revealer", Rc::new(wip::revealer));
    // builder.register_element("root", Rc::new(wip::root));
    // builder.register_element("scaleButton", Rc::new(wip::scale_button));
    builder.register_element("hScrollbar", Rc::new(wip::hscrollbar));
    builder.register_element("vScrollbar", Rc::new(wip::vscrollbar));
    builder.register_element("scrolledWindow", Rc::new(wip::scrolled_window));
    builder.register_element("searchBar", Rc::new(wip::search_bar));
    builder.register_element("searchEntry", Rc::new(wip::search_entry));
    builder.register_element("hSkm eparator", Rc::new(wip::hseparator));
    builder.register_element("vSeparator", Rc::new(wip::vseparator));
    builder.register_element("shortcutLabel", Rc::new(wip::shortcut_label));
    // builder.register_element("shortcutsShortcut", Rc::new(wip::shortcuts_shortcut));
    // builder.register_element("spinButton", Rc::new(wip::spin_button));
    builder.register_element("spinner", Rc::new(wip::spinner));
    builder.register_element("stackSidebar", Rc::new(wip::stack_sidebar));
    builder.register_element("stackSwitcher", Rc::new(wip::stack_switcher));
    builder.register_element("stack", Rc::new(wip::stack));
    builder.register_element("statusbar", Rc::new(wip::statusbar));
    builder.register_element("switch", Rc::new(wip::switch));
    builder.register_element("textView", Rc::new(wip::text_view));
    builder.register_element("text", Rc::new(wip::text));
    builder.register_element("treeExpander", Rc::new(wip::tree_expander));
    builder.register_element("treeView", Rc::new(wip::tree_view));
    builder.register_element("video", Rc::new(wip::video));
    builder.register_element("viewport", Rc::new(wip::viewport));
    builder.register_element("windowControls", Rc::new(wip::window_controls));
    builder.register_element("windowHandle", Rc::new(wip::window_handle));
    builder.register_element(
        "applicationWindow",
        Rc::new(application::application_window),
    );
}
