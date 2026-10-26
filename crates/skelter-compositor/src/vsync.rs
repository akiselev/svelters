//! Platform frame sources on the compositor's single wait. A `skelter_platform::FrameSource` blocks
//! in `wait` until the next frame opportunity, so each window with one gets a small thread that
//! waits on it while the window's frame clock is subscribed and posts what it learns (ticks,
//! occlusion, presentation feedback) into the compositor's channel as messages.

use crate::core::Msg;
use skelter_base::ids::WindowId;
use skelter_base::time::Span;
use skelter_platform::{FrameSource, FrameWait};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

/// How long one `wait` may block, so an unsubscribe or close is seen promptly.
const WAIT_SLICE: Span = Span::from_millis(100);

#[derive(Default)]
struct GateState {
    subscribed: bool,
    closed: bool,
}

/// Lets the compositor start and stop a vsync thread.
#[derive(Default)]
pub struct Gate {
    state: Mutex<GateState>,
    cv: Condvar,
}

impl Gate {
    pub fn set(&self, subscribed: bool) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).subscribed = subscribed;
        self.cv.notify_all();
    }

    pub fn close(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        self.cv.notify_all();
    }

    /// Blocks while unsubscribed; `false` once closed.
    fn wait_subscribed(&self) -> bool {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while !s.subscribed && !s.closed {
            s = self.cv.wait(s).unwrap_or_else(|e| e.into_inner());
        }
        !s.closed
    }
}

/// A window's frame source as the engine holds it.
#[derive(Clone)]
pub struct VsyncLink {
    pub source: Arc<dyn FrameSource>,
    pub gate: Arc<Gate>,
}

/// Starts the vsync thread of `window` (unsubscribed until its frame clock asks).
pub fn spawn(window: WindowId, source: Arc<dyn FrameSource>, tx: Sender<Msg>) -> VsyncLink {
    let gate = Arc::new(Gate::default());
    let link = VsyncLink { source: source.clone(), gate: gate.clone() };
    let name = format!("skelter-vsync-{}", window.0);
    let spawned = std::thread::Builder::new().name(name).spawn(move || {
        while gate.wait_subscribed() {
            let msg = match source.wait(WAIT_SLICE) {
                FrameWait::Vsync { target, interval, .. } => Some(Msg::Vsync { window, target, interval }),
                FrameWait::Occluded(occluded) => Some(Msg::Occluded { window, occluded }),
                FrameWait::TimedOut => None,
            };
            let mut msgs: Vec<Msg> = msg.into_iter().collect();
            for p in source.take_presented() {
                msgs.push(Msg::Presented { window, frame: p.frame, at: p.at, estimated: p.estimated });
            }
            for m in msgs {
                if tx.send(m).is_err() {
                    return;
                }
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("skelter-compositor: cannot spawn the vsync thread of {window:?}: {e}");
    }
    link
}
