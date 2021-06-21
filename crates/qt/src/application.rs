use std::ffi::{c_void, CString};
use std::os::raw::c_char;
use std::pin::Pin;
use std::sync::Arc;

use cpp::{cpp, cpp_class};

cpp! {{
    #include <memory>
    #include <QtCore/QPointer>
    #include <QtCore/QObject>
    #include <QtCore/QEvent>
    #include <QtCore/QAbstractNativeEventFilter>
    #include <QtWidgets/QApplication>

    class EventFilter : public QAbstractNativeEventFilter
    {
        void* ptr;
        public:
            EventFilter(void* ptr) : QAbstractNativeEventFilter(), ptr(ptr) {}

            bool nativeEventFilter(const QByteArray &eventType, void *message, long *) override
            {
                rust!(application_filter_events [ptr : *mut flume::Sender<usize> as "void *"] {
                    tracing::info!("Event received");
                    let ptr = Arc::from_raw(ptr);
                    if let Err(err) = ptr.send(0) {
                        tracing::error!("Failed to send QApplication event notification");
                    }
                    let _ = Arc::into_raw(ptr);
                });
                return true;
            }

            ~EventFilter() {
                rust!(application_drop_event_filter [ptr : *mut flume::Sender<usize> as "void *"] {
                    let arc = Arc::from_raw(ptr);
                    drop(arc);
                    tracing::warn!("Dropping event filter");
                });
            }
    };
}}

cpp_class!(
    /// Wrap a Qt QApplication
    ///
    /// Note that since there can only be one Application in the process, creating two
    /// QApplication at the same time is not allowed. Doing that will panic.
    unsafe struct QApplication as "QPointer<QApplication>"
);

pub struct Application {
    qapp: Pin<Box<QApplication>>,
    rx: flume::Receiver<()>,
}

impl Application {
    /// Create a new QApplication
    pub fn new() -> Application {
        let (tx, rx) = flume::unbounded();
        let tx = Arc::into_raw(Arc::new(tx));
        let mut arguments: Vec<*mut c_char> = std::env::args()
            .map(|arg| CString::new(arg.into_bytes()).expect("argument contains invalid c-string!"))
            .map(|arg| arg.into_raw())
            .collect();
        let argc: i32 = arguments.len() as i32 - 1;
        let argv: *mut *mut c_char = arguments.as_mut_ptr();

        let qapp = cpp!(unsafe [
            argc as "int",
            argv as "char **",
            tx as "void *"
        ] -> Pin<Box<QApplication>> as "QPointer<QApplication>*" {
            // Static variables when used inside function are initialized only once
            static int _argc  = argc;
            static char **_argv = nullptr;
            // this is *real* initialization, and it would also happen only once
            if (_argv == nullptr) {
                // copy the arguments
                _argv = new char *[argc + 1];
                // argv should be null terminated
                _argv[argc] = nullptr;
                for (int i = 0; i < argc; ++i) {
                    _argv[i] = new char[strlen(argv[i]) + 1];
                    strcpy(_argv[i], argv[i]);
                }
            }
            QPointer<QApplication>* ptr = new QPointer<QApplication>();
            *ptr = new QApplication(_argc, _argv);
            (*ptr)->installNativeEventFilter(new EventFilter(tx));
            return ptr;
        });

        // run destructor
        for arg in arguments {
            let _ = unsafe { CString::from_raw(arg) };
        }

        Application { qapp, rx }
    }

    #[tracing::instrument(skip(self))]
    pub async fn exec(self) -> anyhow::Result<()> {
        tracing::info!("Starting app task");
        while let Ok(msg) = self.rx.recv_async().await {
            tracing::info!("Event notifiation received");
            self.process_events();
            self.set_posted_events();
            self.process_events();
        }
        Ok(())
    }

    // pub fn exec(&self) {
    //     let ptr = self.qapp.as_ref();
    //     cpp!(unsafe [ptr as "QPointer<QApplication> *"] {
    //         (*ptr)->exec();
    //     })
    // }

    pub fn set_posted_events(&self) {
        let ptr = self.qapp.as_ref();
        cpp!(unsafe [ptr as "QPointer<QApplication> *"] {
            (*ptr)->sendPostedEvents();
        })
    }

    pub fn process_events(&self) {
        let ptr = self.qapp.as_ref();
        cpp!(unsafe [ptr as "QPointer<QApplication> *"] {
            (*ptr)->processEvents();
        })
    }
}

impl svelters_core::Application for Application {
    fn start(self) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>>>> {
        Box::pin(self.exec())
    }
}
