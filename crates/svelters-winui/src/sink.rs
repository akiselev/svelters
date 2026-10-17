//! Where a native event handler sends its event. Handlers capture only this (two integers), never a
//! COM reference or a JS value, so an element's handlers cannot keep it alive (backend-abi.md,
//! "Native code never holds a JS value"). The backend state is found through a thread-local:
//! WinUI raises events on the UI thread that owns the element.
use svelters_abi::Value;

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
