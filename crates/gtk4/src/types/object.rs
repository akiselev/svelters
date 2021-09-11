use glib::object::Object as GObject;

use crate::inner::*;

define!(|cls: GObject| {
    // let cls = cls.superclass::<Gtk<GObject>>()?;
    props!(cls(this: Gtk<GObject>) {

    });
    methods!(cls(this: Gtk<GObject>) {

    });
    // event!(cls.clicked as SlotOfBool[val](val));
});
