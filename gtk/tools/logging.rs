use tracing::{
    span::{self, Attributes},
    Id, Subscriber,
};
use tracing_subscriber::{layer::Context, registry, Layer};

enum Message {
    Record,
    FollowsFrom,
    Event,
    Close,
}

type Sender = flume::Sender<Message>;
type Receiver = flume::Receiver<Message>;

pub struct TracingLayer {
    tx: Sender,
    handle: std::thread::JoinHandle<()>,
}

impl Default for TracingLayer {
    fn default() -> Self {
        let (tx, rx) = flume::unbounded();
        let handle = std::thread::spawn(move || TracingLayer::task(rx));
        Self { tx, handle }
    }
}

impl TracingLayer {
    pub fn new() -> Self {
        Default::default()
    }

    fn task(rx: Receiver) {}
}

impl<S> Layer<S> for TracingLayer
where
    S: Subscriber + for<'a> registry::LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {}

    fn on_record(
        &self,
        _span: &span::Id,
        _values: &span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_follows_from(
        &self,
        _span: &span::Id,
        _follows: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_enter(&self, _id: &span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}

    fn on_exit(&self, _id: &span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}

    fn on_close(&self, _id: span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}

    fn on_id_change(
        &self,
        _old: &span::Id,
        _new: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        panic!("on_id_change")
    }
}
