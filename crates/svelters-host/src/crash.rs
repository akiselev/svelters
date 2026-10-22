//! DX1 (QS13, core half): what a crash or panic report gets from the JS side. A per-thread ring of
//! the last host ops and of the last reported JS errors (stacks mapped to sources), appended to the
//! panic message by [`install_panic_hook`] and readable with [`context`].

use std::cell::RefCell;
use std::collections::VecDeque;

/// Host ops kept.
const OPS: usize = 32;
/// Reported JS errors kept.
const ERRORS: usize = 8;

#[derive(Default)]
struct Ring {
    ops: VecDeque<&'static str>,
    errors: VecDeque<String>,
}

thread_local! {
    static RING: RefCell<Ring> = RefCell::new(Ring::default());
}

pub(crate) fn note_op(name: &'static str) {
    RING.with(|r| {
        let mut r = r.borrow_mut();
        if r.ops.len() == OPS {
            r.ops.pop_front();
        }
        r.ops.push_back(name);
    });
}

pub(crate) fn note_error(text: String) {
    RING.with(|r| {
        let mut r = r.borrow_mut();
        if r.errors.len() == ERRORS {
            r.errors.pop_front();
        }
        r.errors.push_back(text);
    });
}

/// The JS-side context of this thread's host: the last ops (oldest first) and the last reported
/// JS errors with their mapped stacks. Empty when the thread ran no host.
pub fn context() -> String {
    RING.with(|r| {
        let r = r.borrow();
        if r.ops.is_empty() && r.errors.is_empty() {
            return String::new();
        }
        let mut s = String::from("svelters JS context:\n");
        if !r.errors.is_empty() {
            s.push_str("  last JS errors:\n");
            for e in &r.errors {
                for line in e.lines() {
                    s.push_str("    ");
                    s.push_str(line);
                    s.push('\n');
                }
            }
        }
        if !r.ops.is_empty() {
            s.push_str("  last host ops: ");
            s.push_str(&r.ops.iter().copied().collect::<Vec<_>>().join(", "));
            s.push('\n');
        }
        s
    })
}

/// Chains a panic hook that prints [`context`] after the previous hook's report. Call once.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        prev(info);
        let c = context();
        if !c.is_empty() {
            eprintln!("{c}");
        }
    }));
}
