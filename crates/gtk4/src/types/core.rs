use glib::object::Object as GObject;
use gtk4::{gdk::Cursor, prelude::*, Align, Application, Overflow, TextDirection, Widget};

use crate::inner::*;

define!(|cls: GObject| {
    // let cls = cls.superclass::<Gtk<GObject>>()?;

    // layout_manager: {
    //     layout_manager
    // };
    // fn <P: IsA<LayoutManager>>(&self, layout_manager: Option<&P>)
    // focus_child: {
    //     focus_child
    // };
    // fn <P: IsA<Widget>>(&self, child: Option<&P>)
    // font_map: {
    //     font_map
    // };
    // fn <P: IsA<FontMap>>(&self, font_map: Option<&P>)
    // parent: {
    //     parent
    // };
    // fn <P: IsA<Widget>>(&self, parent: &P)
    props!(cls(this: Gtk<GObject>) {
    });
    methods!(cls(this: Gtk<GObject>) {

    });
    // event!(cls.clicked as SlotOfBool[val](val));
});

define!(|cls: Widget| {
    let cls = cls.superclass::<Gtk<GObject>>()?;

    // layout_manager: {
    //     layout_manager
    // };
    // fn <P: IsA<LayoutManager>>(&self, layout_manager: Option<&P>)
    // focus_child: {
    //     focus_child
    // };
    // fn <P: IsA<Widget>>(&self, child: Option<&P>)
    // font_map: {
    //     font_map
    // };
    // fn <P: IsA<FontMap>>(&self, font_map: Option<&P>)
    // parent: {
    //     parent
    // };
    // fn <P: IsA<Widget>>(&self, parent: &P)
    props!(cls(this: Gtk<Widget>) {


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
        direction:  Gtk<TextDirection> {
            Gtk(this.direction());
            this.set_direction(direction.0);
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
        hAlign: Gtk<Align> {
            Gtk(this.halign());
            this.set_halign(hAlign.0);
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
        overflow:  Gtk<Overflow> {
            Gtk(this.overflow());
            this.set_overflow(overflow.0);
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
        vAlign:  Gtk<Align> {
            Gtk(this.valign());
            this.set_valign(vAlign.0);
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
    methods!(cls(this: Gtk<Widget>) {
        // cursorFromName:  Option<&str> {
        //     this.cursor_from_name();
        //     this.set_cursor_from_name(cursorFromName);
        // };
    });
    // event!(cls.clicked as SlotOfBool[val](val));
});

gtk_enum!(Overflow: "hidden" as Overflow::Hidden, "visible" as Overflow::Visible);
gtk_enum!(Align:
    "fill" as Align::Fill,
    "start" as Align::Start,
    "end" as Align::End,
    "center" as Align::Center,
    "baseline" as Align::Baseline,
);
gtk_enum!(TextDirection:
    "none" as TextDirection::None,
    "ltr" as TextDirection::Ltr,
    "rtl" as TextDirection::Rtl,
);

define!(|cls: Application| {
    let cls = cls.superclass::<Gtk<GObject>>()?;
    props!(cls(this: Gtk<Application>) {

    });
    methods!(cls(this: Gtk<Application>) {

    });
    // event!(cls.clicked as SlotOfBool[val](val));
});

#[rquickjs::bind(object)]
#[quickjs(bare)]
mod appmod {
    use super::*;
    pub struct GtkApplication {
        #[quickjs(skip)]
        pub app: Application,
    }

    impl GtkApplication {
        #[quickjs(skip)]
        pub fn new(app: Application) -> Self {
            Self { app }
        }
    }
}
