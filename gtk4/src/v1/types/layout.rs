use glib::object::Object as GObject;
use gtk4::{
    gdk::Monitor,
    gio,
    gsk::Transform,
    prelude::{LayoutChildExt, LayoutManagerExt},
    Application, ApplicationWindow, ConstraintLayoutChild, FixedLayoutChild, GridLayoutChild,
    LayoutChild, LayoutManager, Orientation, OverlayLayoutChild, SizeRequestMode, Widget,
    WindowGroup,
};

use crate::inner::*;

define!(|cls: Transform| {
    let cls = cls.constructor0(|| Ok(Gtk(Transform::new())))?;
    props!(cls(this: Gtk<Transform>) {});
});

define!(|cls: LayoutChild| {
    let cls = cls.superclass::<Gtk<GObject>>()?;
    props!(cls(this: Gtk<LayoutChild>) {
        childWidget: Option<Impl<Gtk<Widget>>> {
            this.child_widget().map(Gtk).map(Impl::new)
        };
        layoutManager: Option<Impl<Gtk<LayoutManager>>> {
            this.layout_manager().map(Gtk).map(Impl::new)
        };
    });
});

define!(|cls: ConstraintLayoutChild| {
    let cls = cls.superclass::<Gtk<LayoutChild>>()?;
});

define!(|cls: FixedLayoutChild| {
    let cls = cls.superclass::<Gtk<LayoutChild>>()?;
    props!(cls(this: Gtk<FixedLayoutChild>) {
        transform: Option<Impl<Gtk<Transform>>> {
            this.transform().map(Gtk).map(Impl::new);
            this.set_transform(&transform.unwrap_or_else(|| Impl::new(Gtk(Transform::new()))).0);
        };
    });
});

define!(|cls: GridLayoutChild| {
    let cls = cls.superclass::<Gtk<LayoutChild>>()?;

    props!(cls(this: Gtk<GridLayoutChild>) {
        row: i32 {
            this.row();
            this.set_row(row);
        };
        rowSpan: i32 {
            this.row_span();
            this.set_row_span(rowSpan);
        };
        column: i32 {
            this.column();
            this.set_column(column);
        };
        columnSpan: i32 {
            this.column_span();
            this.set_column_span(columnSpan);
        };
    });
});

define!(|cls: OverlayLayoutChild| {
    let cls = cls.superclass::<Gtk<LayoutChild>>()?;

    props!(cls(this: Gtk<OverlayLayoutChild>) {
        measure: bool {
            this.is_measure();
            this.set_measure(measure);
        };
        clipOverlay: bool {
            this.is_clip_overlay();
            this.set_clip_overlay(clipOverlay);
        };
    });
});

define!(|cls: LayoutManager| {
    let cls = cls.superclass::<Gtk<GObject>>()?;
    props!(cls(this: Gtk<LayoutManager>) {
        requestMode: Enum<SizeRequestMode> {
            this.request_mode().into();
        };
        widget: Option<Impl<Gtk<Widget>>> {
            this.widget().map(Gtk).map(Impl::new)
        };
    });

    methods!(cls(this: Gtk<LayoutManager>) {
        allocate[4](widget: Impl<Gtk<Widget>>, width: i32, height: i32, baseline: i32) {
            this.allocate(&widget.0, width, height, baseline)
        };
        measure[3](widget: Impl<Gtk<Widget>>, orientation: Enum<Orientation>, for_size: i32) {
            this.measure(&widget.0, orientation.into(), for_size)
        };
        layoutChild[1](widget: Impl<Gtk<Widget>>) {
            this.layout_child(&widget.0).map(Gtk).map(Impl::new)
        };
        layoutChanged[0]() {
            this.layout_changed()
        };
    });
    // event!(cls.clicked as SlotOfBool[val](val));
});
