//! Streams from Rust to JS (E3, `docs/notes/egui-rust-exports.md` §6.6): a [`Sink<T>`] parameter of an
//! offloaded export, or a [`Stream<T>`] result, becomes an async iterable in JS.
//!
//! ```ignore
//! #[svelters::export]
//! impl Simulation {
//!     /// JS: `for await (const f of sim.run(600, { signal })) ...`
//!     #[js(worker, buffer = 4)]
//!     pub fn run(self: Arc<Self>, frames: u32, out: Sink<Frame>, cx: WorkCx) -> Result<(), NativeError> {
//!         for _ in 0..frames { out.send(self.step())?; }   // blocks while 4 frames are unread
//!         Ok(())
//!     }
//!     /// A stream fed from elsewhere (a service): the call returns at once.
//!     pub fn samples(&self) -> Stream<Sample> { let (tx, rx) = channel(16); self.feeds.add(tx); rx }
//! }
//! ```
//!
//! - **Bounded.** At most `buffer` items (default 8) are unread: queued in Rust or handed to JS but not
//!   yet taken by the iterator. `send` blocks (and `feed(..).await` waits) while the stream is full.
//! - **Delivery** is at checkpoints: every queued item reaches the JS iterator in one engine entry per
//!   checkpoint (with the offload completions); `stream.batches()` yields them as arrays.
//! - **Closing.** Leaving a `for await` early (`return()`), an abort of `options.signal`, a hot reload or
//!   the host going away closes the stream: the producer's next `send` returns `Err(Closed)`.
//! - **The end.** A `Sink` stream ends when its call returns (`Err` rejects the iterator after the items
//!   already sent); a returned `Stream` ends when every `Sink` of its channel is dropped, or with
//!   [`Sink::fail`].

use super::offload::{Completions, OpShared};
use super::{CallCx, IntoJs, JsType, NativeError, TypeBuilder, TypeRef, Value, host_error};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};

/// Items a stream holds unread when the export does not say `#[js(buffer = n)]`.
pub const DEFAULT_BUFFER: usize = 8;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The JS side closed the stream (the loop ended early, the call was aborted, the runtime reloaded).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the stream was closed by its reader")
    }
}

impl std::error::Error for Closed {}

impl From<Closed> for NativeError {
    fn from(_: Closed) -> NativeError {
        host_error("cancelled", "the stream was closed by its reader")
    }
}

impl From<Closed> for String {
    fn from(c: Closed) -> String {
        c.to_string()
    }
}

impl super::ExportError for Closed {
    fn code(&self) -> String {
        "cancelled".into()
    }
    fn message(&self) -> String {
        self.to_string()
    }
    fn into_native(self, _domain: &str) -> NativeError {
        self.into()
    }
}

/// Why [`Sink::try_send`] did not queue the item.
#[derive(Debug, PartialEq, Eq)]
pub enum TrySendError<T> {
    /// `buffer` items are unread.
    Full(T),
    Closed(T),
}

struct State<T> {
    queue: VecDeque<T>,
    /// Items queued or handed to JS and not yet taken by the iterator.
    unread: usize,
    closed: bool,
    senders: usize,
    failed: Option<NativeError>,
    /// Async senders waiting for room.
    feeders: Vec<Waker>,
    /// The end future of a returned `Stream`.
    end: Option<Waker>,
    /// The call the stream belongs to: (op id, its completion queue).
    bound: Option<(u64, Arc<Completions>)>,
}

/// The channel between the sinks and the host.
pub(crate) struct Shared<T> {
    st: Mutex<State<T>>,
    room: Condvar,
    cap: usize,
    /// The host was told about queued items and has not taken them yet.
    notified: AtomicBool,
}

impl<T> Shared<T> {
    fn new(cap: usize, senders: usize) -> Arc<Shared<T>> {
        Arc::new(Shared {
            st: Mutex::new(State { queue: VecDeque::new(), unread: 0, closed: false, senders, failed: None, feeders: Vec::new(), end: None, bound: None }),
            room: Condvar::new(),
            cap: cap.max(1),
            notified: AtomicBool::new(false),
        })
    }

    /// Tells the host there are items (once until it takes them). Called without the state lock.
    fn notify(&self, bound: Option<(u64, Arc<Completions>)>) {
        if let Some((id, q)) = bound {
            if !self.notified.swap(true, Ordering::AcqRel) {
                q.push_items(id);
            }
        }
    }

    fn close(&self) {
        let (feeders, end) = {
            let mut st = lock(&self.st);
            st.closed = true;
            st.queue.clear();
            (std::mem::take(&mut st.feeders), st.end.take())
        };
        self.room.notify_all();
        feeders.into_iter().for_each(Waker::wake);
        if let Some(w) = end {
            w.wake();
        }
    }

    fn sender_gone(&self) {
        let (end, bound) = {
            let mut st = lock(&self.st);
            st.senders = st.senders.saturating_sub(1);
            if st.senders > 0 {
                return;
            }
            (st.end.take(), st.bound.clone())
        };
        if let Some(w) = end {
            w.wake();
        }
        // Wake the host so a finished stream's tail is delivered.
        self.notify(bound);
    }
}

/// The type-erased stream an op carries (the host's side).
pub(crate) trait StreamSource: Send + Sync {
    /// Every queued item, converted on the JS thread.
    fn take(&self, cx: &CallCx<'_>) -> Result<Vec<Value>, NativeError>;
    /// The JS iterator took `n` items: room for the producer.
    fn ack(&self, n: usize);
    /// The reader went away.
    fn close(&self);
}

impl<T: IntoJs + Send + 'static> StreamSource for Shared<T> {
    fn take(&self, cx: &CallCx<'_>) -> Result<Vec<Value>, NativeError> {
        self.notified.store(false, Ordering::Release);
        let items: Vec<T> = lock(&self.st).queue.drain(..).collect();
        items.into_iter().map(|i| i.into_js(cx)).collect()
    }

    fn ack(&self, n: usize) {
        let feeders = {
            let mut st = lock(&self.st);
            st.unread = st.unread.saturating_sub(n);
            std::mem::take(&mut st.feeders)
        };
        self.room.notify_all();
        feeders.into_iter().for_each(Waker::wake);
    }

    fn close(&self) {
        Shared::close(self);
    }
}

impl<T: IntoJs + Send + 'static> Shared<T> {
    /// Attaches the stream to op `op`: its items go to that call's iterator, and cancelling the call
    /// (break, abort, reload) closes the stream.
    fn bind(self: &Arc<Self>, op: &Arc<OpShared>) {
        let bound = Some((op.id, op.queue.clone()));
        let pending = {
            let mut st = lock(&self.st);
            st.bound = bound.clone();
            !st.queue.is_empty() || st.senders == 0
        };
        let weak = Arc::downgrade(self);
        op.on_cancel(Box::new(move || {
            if let Some(s) = weak.upgrade() {
                s.close();
            }
        }));
        op.set_stream(self.clone());
        if pending {
            self.notify(bound);
        }
    }
}

/// The producing end of a stream: `Send`, cloneable (the stream ends when the last clone of a returned
/// [`Stream`]'s sink is dropped).
pub struct Sink<T> {
    s: Arc<Shared<T>>,
}

impl<T> Clone for Sink<T> {
    fn clone(&self) -> Self {
        lock(&self.s.st).senders += 1;
        Sink { s: self.s.clone() }
    }
}

impl<T> Drop for Sink<T> {
    fn drop(&mut self) {
        self.s.sender_gone();
    }
}

impl<T> std::fmt::Debug for Sink<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sink").field("buffer", &self.s.cap).finish()
    }
}

impl<T: Send + 'static> Sink<T> {
    /// Queues `item`, blocking while `buffer` items are unread. `Err(Closed)` once the reader is gone.
    /// Never call it on the JS thread (a JS-thread `async fn` uses [`Sink::feed`]): the reader runs there.
    pub fn send(&self, item: T) -> Result<(), Closed> {
        let mut st = lock(&self.s.st);
        loop {
            if st.closed {
                return Err(Closed);
            }
            if st.unread < self.s.cap {
                st.queue.push_back(item);
                st.unread += 1;
                let bound = st.bound.clone();
                drop(st);
                self.s.notify(bound);
                return Ok(());
            }
            st = self.s.room.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Queues `item` if there is room, without waiting.
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        let mut st = lock(&self.s.st);
        if st.closed {
            return Err(TrySendError::Closed(item));
        }
        if st.unread >= self.s.cap {
            return Err(TrySendError::Full(item));
        }
        st.queue.push_back(item);
        st.unread += 1;
        let bound = st.bound.clone();
        drop(st);
        self.s.notify(bound);
        Ok(())
    }

    /// Queues `item`, waiting (asynchronously) while the stream is full: for `async` producers.
    pub fn feed(&self, item: T) -> Feed<'_, T> {
        Feed { sink: self, item: Some(item) }
    }

    /// Ends the stream with an error: the iterator rejects with it after the items already sent.
    pub fn fail(&self, err: impl Into<NativeError>) {
        let (end, bound) = {
            let mut st = lock(&self.s.st);
            st.failed = Some(err.into());
            (st.end.take(), st.bound.clone())
        };
        if let Some(w) = end {
            w.wake();
        }
        self.s.notify(bound);
    }

    /// True once the reader is gone.
    pub fn is_closed(&self) -> bool {
        lock(&self.s.st).closed
    }

    /// The `buffer` of the stream.
    pub fn capacity(&self) -> usize {
        self.s.cap
    }
}

/// [`Sink::feed`].
pub struct Feed<'a, T> {
    sink: &'a Sink<T>,
    item: Option<T>,
}

impl<T> Unpin for Feed<'_, T> {}

impl<T: Send + 'static> Future for Feed<'_, T> {
    type Output = Result<(), Closed>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let s = self.sink.s.clone();
        let mut st = lock(&s.st);
        if st.closed {
            return Poll::Ready(Err(Closed));
        }
        if st.unread < s.cap {
            let Some(item) = self.item.take() else { return Poll::Ready(Ok(())) };
            st.queue.push_back(item);
            st.unread += 1;
            let bound = st.bound.clone();
            drop(st);
            s.notify(bound);
            return Poll::Ready(Ok(()));
        }
        st.feeders.push(cx.waker().clone());
        Poll::Pending
    }
}

/// The reading end of a stream, returned by an export: an async iterable in JS. Make one with
/// [`channel`]; keep the [`Sink`]s where the items come from (a service, a worker).
pub struct Stream<T> {
    s: Arc<Shared<T>>,
}

/// A stream with room for `buffer` unread items.
pub fn channel<T: IntoJs + Send + 'static>(buffer: usize) -> (Sink<T>, Stream<T>) {
    let s = Shared::new(buffer, 1);
    (Sink { s: s.clone() }, Stream { s })
}

impl<T> std::fmt::Debug for Stream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream").field("buffer", &self.s.cap).finish()
    }
}

/// The item type of a stream member's typings (`NativeStream<T>`).
pub trait StreamItem: IntoJs + JsType + Send + 'static {}
impl<T: IntoJs + JsType + Send + 'static> StreamItem for T {}

/// Completes when every sink of a returned stream is gone (`Ok`), or with [`Sink::fail`]'s error.
struct End<T> {
    s: Arc<Shared<T>>,
}

impl<T> Future for End<T> {
    type Output = Result<(), NativeError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = lock(&self.s.st);
        if let Some(e) = st.failed.take() {
            return Poll::Ready(Err(e));
        }
        if st.senders == 0 || st.closed {
            return Poll::Ready(Ok(()));
        }
        st.end = Some(cx.waker().clone());
        Poll::Pending
    }
}

// ---- what the macros call ----

/// A `Sink<T>` parameter of an offloaded export: a fresh stream bound to the call (`#[js(buffer = n)]`).
#[doc(hidden)]
pub fn sink_for<T: IntoJs + Send + 'static>(op: &Arc<OpShared>, buffer: usize) -> Sink<T> {
    let s = Shared::new(buffer, 1);
    s.bind(op);
    Sink { s }
}

/// A `Stream<T>` result: the call's job waits on the JS thread until the stream ends.
#[doc(hidden)]
pub fn stream_job<T: IntoJs + Send + 'static>(stream: Stream<T>, op: &Arc<OpShared>) -> super::offload::Job {
    stream.s.bind(op);
    let end = End { s: stream.s.clone() };
    super::offload::local_job::<()>(async move {
        // Keep the reading end alive as long as the call is.
        let _keep = stream;
        end.await
    })
}

/// The typings of a stream member's items.
#[doc(hidden)]
pub fn item_type<T: JsType>(b: &mut TypeBuilder) -> Option<TypeRef> {
    Some(T::describe(b))
}
