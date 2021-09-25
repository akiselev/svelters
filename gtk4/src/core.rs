use glib::object::Object as GObject;
use gtk4::{
    gdk::Cursor, prelude::*, Align, Application, ConstraintTarget, Overflow, TextDirection, Widget,
    Window,
};

use crate::inner::*;

pub(in crate) fn object_interface(this: &mut ElementDef) {}

pub(in crate) fn widget_interface(this: &mut ElementDef) {
    props!(|this: Widget| {
        canFocus:  bool {
            this.can_focus();
            this.set_can_focus(canFocus);
        };
        canTarget:  bool {
            this.can_target();
            this.set_can_target(canTarget);
        };
        childVisible:  bool {
            this.is_child_visible();
            this.set_child_visible(childVisible);
        };
        cssClasses:  Vec<String> {
            this.css_classes().iter().map(|cls| cls.as_str().to_string()).collect();
            {
                let classes: Vec<&str> =  cssClasses.iter().map(String::as_str).collect();
                this.set_css_classes(&classes);
            };
        };
        // cursor:  Option<&Cursor> {
        //     this.cursor();
        //     this.set_cursor(cursor);
        // };
        direction:  Enum<TextDirection> {
            this.direction().into();
            this.set_direction(direction.into());
        };
        focusOnClick:  bool {
            this.gets_focus_on_click();
            this.set_focus_on_click(focusOnClick);
        };
        focusable:  bool {
            this.is_focusable();
            this.set_focusable(focusable);
        };
        // fontOptions:  Option<&FontOptions> {
        //     this.font_options();
        //     this.set_font_options(fontOptions);
        // };
        hAlign: Enum<Align> {
            this.halign().into();
            this.set_halign(hAlign.into());
        };
        hasTooltip:  bool {
            this.has_tooltip();
            this.set_has_tooltip(hasTooltip);
        };
        heightRequest:  i32 {
            this.height_request();
            this.set_height_request(heightRequest);
        };
        hExpandSet:  bool {
            this.is_hexpand_set();
            this.set_hexpand_set(hExpandSet);
        };
        hExpands:  bool {
            this.hexpands();
            this.set_hexpand(hExpands);
        };
        marginBottom:  i32 {
            this.margin_bottom();
            this.set_margin_bottom(marginBottom);
        };
        marginEnd:  i32 {
            this.margin_end();
            this.set_margin_end(marginEnd);
        };
        marginStart:  i32 {
            this.margin_start();
            this.set_margin_start(marginStart);
        };
        marginTop:  i32 {
            this.margin_top();
            this.set_margin_top(marginTop);
        };
        opacity:  f64 {
            this.opacity();
            this.set_opacity(opacity);
        };
        overflow:  Enum<Overflow> {
            this.overflow().into();
            this.set_overflow(overflow.into());
        };
        receivesDefault:  bool {
            this.receives_default();
            this.set_receives_default(receivesDefault);
        };
        sensitive:  bool {
            this.is_sensitive();
            this.set_sensitive(sensitive);
        };
        sizeRequest:  (i32, i32) {
            this.size_request();
            this.set_size_request(sizeRequest.0, sizeRequest.1);
        };
        tooltipMarkup:  Option<String> {
            this.tooltip_markup().map(|val| val.to_string());
            this.set_tooltip_markup(tooltipMarkup.as_ref().map(String::as_str));
        };
        tooltipText:  Option<String> {
            this.tooltip_text().map(|val| val.to_string());
            this.set_tooltip_text(tooltipText.as_ref().map(String::as_str));
        };
        vAlign:  Enum<Align> {
            this.valign().into();
            this.set_valign(vAlign.into());
        };
        vExpandSet:  bool {
            this.is_vexpand_set();
            this.set_vexpand_set(vExpandSet);
        };
        vExpands:  bool {
            this.vexpands();
            this.set_vexpand(vExpands);
        };
        visible:  bool {
            this.is_visible();
            this.set_visible(visible);
        };
        widgetName:  String {
            this.widget_name().to_string();
            this.set_widget_name(&widgetName);
        };
        widthRequest:  i32 {
            this.width_request();
            this.set_width_request(widthRequest);
        };
    });
}

pub(in crate) fn accessible_interface(this: &mut ElementDef) {}

pub(in crate) fn window_interface(this: &mut ElementDef) {
    props!(|this: Window| {
        decorated: bool {
            this.is_decorated();
            this.set_decorated(decorated);
        };
        defaultSize: (i32, i32) {
            this.default_size();
            this.set_default_size(defaultSize.0, defaultSize.1);
        };
        deletable: bool {
            this.is_deletable();
            this.set_deletable(deletable);
        };
        destroyWithParent: bool {
            this.must_destroy_with_parent();
            this.set_destroy_with_parent(destroyWithParent);
        };
        focusVisible: bool {
            this.gets_focus_visible();
            this.set_focus_visible(focusVisible);
        };
        handleMenubarAccel: bool {
            this.is_handle_menubar_accel();
            this.set_handle_menubar_accel(handleMenubarAccel);
        };
        mnemonicsVisible: bool {
            this.is_mnemonics_visible();
            this.set_mnemonics_visible(mnemonicsVisible);
        };
        modal: bool {
            this.is_modal();
            this.set_modal(modal);
        };
        resizable: bool {
            this.is_resizable();
            this.set_resizable(resizable);
        };
        maximized: bool {
            this.is_maximized();
            this.set_maximized(maximized);
        };
        defaultWidth: i32 {
            this.default_width();
            this.set_default_width(defaultWidth);
        };
        defaultHeight: i32 {
            this.default_height();
            this.set_default_height(defaultHeight);
        };
        fullscreened: bool {
            this.is_fullscreened();
            this.set_fullscreened(fullscreened);
        };
        title: Option<String> {
            this.title().map(|val| val.to_string());
            this.set_title(title.as_ref().map(String::as_str));
        };
        iconName: Option<String> {
            this.icon_name().map(|val| val.to_string());
            this.set_icon_name(iconName.as_ref().map(String::as_str));
        };
        hideOnClose: bool {
            this.hides_on_close();
            this.set_hide_on_close(hideOnClose);
        };
    });
}

pub(in crate) fn application_window_interface(this: &mut ElementDef) {}
