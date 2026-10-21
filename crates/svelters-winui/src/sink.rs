//! Where a native event handler sends its event. Handlers capture only this (two integers), never a
//! COM reference or a JS value, so an element's handlers cannot keep it alive (backend-abi.md,
//! "Native code never holds a JS value"). The backend state is found through a thread-local:
//! WinUI raises events on the UI thread that owns the element.
use crate::bindings::Microsoft::UI::Xaml::{DependencyObject, DependencyProperty, DependencyPropertyChangedCallback};
use svelters_abi::Value;
use windows_core::{EventRevoker, IInspectable, Interface};

/// A live native subscription; dropping it unsubscribes.
pub enum Registration {
    /// A WinRT event (`Click`, `Closed`, ...). Held for its `Drop`, which revokes the handler.
    Event(#[allow(dead_code)] EventRevoker),
    /// A `notify::<prop>` listener: `DependencyObject.RegisterPropertyChangedCallback`, undone by
    /// `UnregisterPropertyChangedCallback` with the same property and token. The object reference is
    /// the element itself, which the handle table already holds.
    Property { obj: DependencyObject, dp: DependencyProperty, token: i64 },
    /// N4: an event of a type object through the dynamic invoker: `remove_X(token)` on the event's interface.
    Dynamic { iface: windows_core::IUnknown, remove: u16, token: i64 },
}

impl From<EventRevoker> for Registration {
    fn from(r: EventRevoker) -> Self {
        Registration::Event(r)
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        match self {
            Registration::Property { obj, dp, token } => {
                let _ = obj.UnregisterPropertyChangedCallback(&*dp, *token);
            }
            Registration::Dynamic { iface, remove, token } => {
                // SAFETY: `remove_X(EventRegistrationToken)`: the 8-byte token passes as an integer.
                let _ = unsafe { crate::types::rt::vcall(iface, *remove, &[crate::types::invoke::Word::I(*token as u64)]) };
            }
            Registration::Event(_) => {}
        }
    }
}

/// `notify::<prop>`: post an argument-less event whenever the DependencyProperty `dp` of `o` changes
/// (programmatic or user change; the callback carries no value, the listener reads the prop).
pub fn notify(o: &IInspectable, dp: &DependencyProperty, sink: Sink) -> windows_core::Result<Registration> {
    let obj = o.cast::<DependencyObject>()?;
    let cb = DependencyPropertyChangedCallback::new(move |_, _| {
        sink.post(Vec::new());
        Ok(())
    });
    let token = obj.RegisterPropertyChangedCallback(dp, &cb)?;
    Ok(Registration::Property { obj, dp: dp.clone(), token })
}

/// ABI v2.6 `observe` of a DependencyProperty: `host.prop_changed(h, prop)` on every change callback. The delegate
/// captures the two ids only; the backend is reached through its thread-local weak reference, so an observation keeps
/// neither the backend nor the element alive (the registration holds the element, which the handle table already does).
pub fn observe_dp(o: &IInspectable, dp: &DependencyProperty, h: u64, prop: u32) -> windows_core::Result<Registration> {
    let obj = o.cast::<DependencyObject>()?;
    let cb = DependencyPropertyChangedCallback::new(move |_, _| {
        crate::backend::emit_changed(h, prop);
        Ok(())
    });
    let token = obj.RegisterPropertyChangedCallback(dp, &cb)?;
    Ok(Registration::Property { obj, dp: dp.clone(), token })
}

#[derive(Clone, Copy, Debug)]
pub struct Sink {
    pub h: u64,
    /// Event id (index + 1 into `EVENTS`) with listen flags in the high bits.
    pub ev: u32,
}

impl Sink {
    /// Queue the event for the next checkpoint.
    pub fn post(&self, args: Vec<Value>) {
        crate::backend::emit_post(*self, &args);
    }

    /// Run the JS listeners now; the reply (`e.preventDefault()` -> `true`), or `None` when the
    /// host was busy or failed (the toolkit default applies).
    pub fn sync(&self, args: Vec<Value>) -> Option<Value> {
        crate::backend::emit_sync(*self, &args)
    }

    /// A close request on a toplevel. `true`: veto it (JS listens and was told).
    pub fn close(&self) -> bool {
        crate::backend::emit_close(*self)
    }
}
