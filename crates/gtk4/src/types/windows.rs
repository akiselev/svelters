use glib::object::Object as GObject;
use gtk4::{
    gdk::Monitor, gio, prelude::GtkWindowExt, Application, ApplicationWindow, Widget, Window,
    WindowGroup,
};

use crate::inner::*;

define!(|cls: Window| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .constructor0(|| Ok(Gtk(Window::new())))?;
    props!(cls(this: Gtk<Window>) {
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
            this.0.is_handle_menubar_accel();
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
        active: bool {
            this.is_active();
        };
        hasGroup: bool {
            this.has_group();
        };
        // group: Gtk<WindowGroup> {
        //     Gtk(this.group());
        // };
        // application: Option<Virt<Gtk<Application>>> {
        //     this.application().map(Gtk);
        //     this.set_application(application);
        // };
    });

    methods!(cls(this: Gtk<Window>) {
        destroy[0]() { this.destroy() };
        close[0]() { this.close() };
        present[0]() { this.present() };
        fullscreen[0]() { this.fullscreen() };
        minimize[0]() { this.minimize() };
        maximize[0]() { this.maximize() };
        unfullscreen[0]() { this.unfullscreen() };
        unminimize[0]() { this.unminimize() };
        unmaximize[0]() { this.unmaximize() };
        // unmaximize[1](monitor: Virt<Gtk<Monitor>>) { this.fullscreen_on_monitor(&monitor.0) };


        // startupId: &str {
        //     this.startup_id();
        //     this.set_startup_id(startupId);
        // };
        // child: Option<Virt<Gtk<Widget>>> {
        //     this.child();
        //     this.set_child(child);
        // };
        // focusWidget: Option<Virt<Gtk<Widget>>> {
        //     this.focus_widget();
        //     this.set_focus_widget(focusWidget);
        // };
        // defaultWidget: Option<Virt<Gtk<Widget>>> {
        //     this.default_widget();
        //     this.set_default_widget(defaultWidget);
        // };
        // display: Virt<Gtk<Display>> {
        //     this.display();
        //     this.set_display(display);
        // };
        // titlebar: Option<Virt<Gtk<Widget>>> {
        //     this.titlebar();
        //     this.set_titlebar(titlebar);
        // };
        // transientFor: Option<Virt<Gtk<Window>>> {
        //     this.transient_for();
        //     this.set_transient_for(transientFor);
        // };
    });
    // event!(cls.clicked as SlotOfBool[val](val));
});

define!(|cls: ApplicationWindow| {
    let cls =
        cls.superclass::<Gtk<Widget>>()?
            .as_element_with_js("applicationWindow", |ctx: Ctx| {
                let app: &Obj<Gtk<Application>> = ctx.globals().get("application")?;
                let inner = app.borrow()?;
                Ok(Gtk(ApplicationWindow::new(&inner.0)))
            })?;
    props!(cls(this: Gtk<ApplicationWindow>) {

    });
    methods!(cls(this: Gtk<ApplicationWindow>) {

    });
    // event!(cls.clicked as SlotOfBool[val](val));
});
