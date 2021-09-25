use crate::inner::*;

use glib::object::Object as GObject;
use gtk4::{prelude::*, *};

define!(|cls: Accessible| {
    let cls = cls.superclass::<Gtk<GObject>>()?;
    props!(cls(this: Gtk<Accessible>) {
        accessibleRole: Enum<AccessibleRole> {
            this.accessible_role().into();
            this.set_accessible_role(accessibleRole.into());
        };
        // group: Gtk<WindowGroup> {
        //     Gtk(this.group());
        // };
        // application: Option<Virt<Gtk<Application>>> {
        //     this.application().map(Gtk);
        //     this.set_application(application);
        // };
    });

    methods!(cls(this: Gtk<Accessible>) {

        // child: Option<Virt<Gtk<Widget>>> {
        //     this.child();
        //     this.set_child(child);
        // };
    });
    // event!(cls.clicked as SlotOfBool[val](val));
});
