use glib::object::Object as GObject;
use gtk4::{
    gdk::Monitor, gio, prelude::ButtonExt, Application, ApplicationWindow, Button, Widget,
    WindowGroup,
};

use crate::inner::*;

define!(|cls: Button| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .constructor0(|| Ok(Gtk(Button::new())))?
        .as_element("button", || Ok(Gtk(Button::new())))?;
    props!(cls(this: Gtk<Button>) {
        label: Option<String> {
            this.label().map(|val| val.to_string());
            this.set_label(label.as_ref().map(String::as_str).unwrap_or(""));
        };
        iconName: Option<String> {
            this.icon_name().map(|val| val.to_string());
            this.set_icon_name(iconName.as_ref().map(String::as_str).unwrap_or(""));
        };
        underline: bool {
            this.uses_underline();
            this.set_use_underline(underline);
        };
        hasFrame: bool {
            this.has_frame();
            this.set_has_frame(hasFrame);
        };
        // group: Gtk<WindowGroup> {
        //     Gtk(this.group());
        // };
        // application: Option<Virt<Gtk<Application>>> {
        //     this.application().map(Gtk);
        //     this.set_application(application);
        // };
    });

    methods!(cls(this: Gtk<Button>) {

        // child: Option<Virt<Gtk<Widget>>> {
        //     this.child();
        //     this.set_child(child);
        // };
    });
    // event!(cls.clicked as SlotOfBool[val](val));
});
