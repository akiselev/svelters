use std::rc::Rc;

use glib::{ObjectExt, SignalHandlerId};

use crate::inner::*;

pub struct GtkEvent<IMPL: ?Sized, CONN, DISC> {
    interface: &'static str,
    name: &'static str,
    func: Rc<CONN>,
    disc: Rc<DISC>,
    marker: PhantomData<IMPL>,
}

impl<CONN, DISC> GtkEvent<(), CONN, DISC> {
    #[inline(always)]
    pub fn new<IMPL: 'static + ?Sized>(
        interface: &'static str,
        name: &'static str,
        func: CONN,
        disc: DISC,
    ) -> GtkEvent<IMPL, CONN, DISC>
    where
        IMPL: 'static,
        CONN: 'static + for<'js> Fn(&'js IMPL, Subscription) -> Result<SignalHandlerId>,
        DISC: 'static + for<'js> Fn(&'js IMPL, SignalHandlerId) -> Result<()>,
    {
        GtkEvent {
            name,
            func: Rc::new(func),
            disc: Rc::new(disc),
            interface,
            marker: PhantomData,
        }
    }
}

impl<IMPL, CONN, DISC> IEvent for GtkEvent<IMPL, CONN, DISC>
where
    IMPL: 'static + ?Sized,
    CONN: 'static + for<'js> Fn(&'js IMPL, Subscription) -> Result<SignalHandlerId>,
    DISC: 'static + for<'js> Fn(&'js IMPL, SignalHandlerId) -> Result<()>,
{
    fn subscribe<'js>(
        &self,
        element: Element<'js>,
        func: rquickjs::Function<'js>,
    ) -> svelters_core::prelude::Result<()> {
        let subscriber = self.func.clone();
        let disconnect = self.disc.clone();
        let name = self.name;
        let interface = self.interface;
        element
            .event(self.name)?
            .subscribe(func)
            .connect(move |this: &IMPL, sub, dc| {
                tracing::info!("Connecting event {}::{}", interface, name);
                let conn = (subscriber)(this, sub)?;
                dc.with(move |this: &IMPL, _sub| {
                    (disconnect)(this, conn);
                    Ok(())
                })
            })
    }
}
