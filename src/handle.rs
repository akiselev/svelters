use std::{cell::UnsafeCell, ffi::c_void};

use rquickjs::{ExecutorSpawner, IntoAtom};
use std::future;

use crate::{js::*, SvEvent, Svelters};

use SvEvent::*;

thread_local! {
    static TX: UnsafeCell<Option<flume::Sender<SvEvent>>> = UnsafeCell::new(None);
}

type Result<T, E = flume::SendError<SvEvent>> = std::result::Result<T, E>;

#[derive(Clone)]
pub struct SvHandle {
    tx: flume::Sender<SvEvent>,
}

impl SvHandle {
    pub(crate) fn new(tx: flume::Sender<SvEvent>) -> SvHandle {
        SvHandle { tx }
    }

    pub fn gc(&self) -> Result<()> {
        self.tx.send(SvEvent::Gc)
    }

    pub fn shutdown(&self) -> Result<()> {
        self.tx.send(SvEvent::Shutdown)
    }

    pub fn updated(&self, event: hotwatch::Event) -> Result<()> {
        self.tx.send(Updated(event))
    }

    pub(crate) fn schedule<F>(&self, func: F) -> Result<()>
    where
        F: 'static + FnOnce(&Svelters) -> rquickjs::Result<()>,
    {
        self.tx.send(ScheduleCallback(Box::new(func)))
    }

    pub fn schedule_ffi_callback(
        &self,
        func: unsafe extern "C" fn(*mut c_void),
        data: *mut c_void,
    ) -> Result<()> {
        self.schedule(move |_| unsafe {
            func(data);
            Ok(())
        })
    }

    pub fn schedule_gc(&self) -> Result<()> {
        self.schedule(move |sv| {
            sv.gc();
            Ok(())
        })
    }
}
