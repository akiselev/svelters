use std::ffi::CString;
use std::os::raw::c_char;

use cpp::{cpp, cpp_class};

cpp! {{
    #include <memory>
    #include <QtCore/QPointer>
    #include <QtWidgets/QApplication>

    #ifdef _WIN32
        # define QODE_EXPORT __declspec(dllexport)
    #else
        # define QODE_EXPORT __attribute__((visibility("default")))
    #endif

    namespace qode {
        QODE_EXPORT typedef int (*QodeCustomRunLoopFunc)();
        extern QodeCustomRunLoopFunc custom_run_loop;

        QODE_EXPORT extern void InjectCustomRunLoop(QodeCustomRunLoopFunc customRunLoop);
    }

}}

cpp_class!(
    /// Wrap a Qt QApplication
    ///
    /// Note that since there can only be one Application in the process, creating two
    /// QApplication at the same time is not allowed. Doing that will panic.
    pub unsafe struct QApplication as "QPointer<QApplication>"
);
impl QApplication {
    /// Create a new MainWindow
    pub fn new() -> QApplication {
        let mut arguments: Vec<*mut c_char> = std::env::args()
            .map(|arg| CString::new(arg.into_bytes()).expect("argument contains invalid c-string!"))
            .map(|arg| arg.into_raw())
            .collect();
        let argc: i32 = arguments.len() as i32 - 1;
        let argv: *mut *mut c_char = arguments.as_mut_ptr();
        let result = cpp!(unsafe [
            argc as "int",
            argv as "char **"
        ] -> QApplication as "QPointer<QApplication>" {
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
            return new QApplication(_argc, _argv);
        });

        // run destructor
        for arg in arguments {
            let _ = unsafe { CString::from_raw(arg) };
        }

        result
    }

    pub fn exec(&self) {
        cpp!(unsafe [self as "QPointer<QApplication> *"] {
            qode::InjectCustomRunLoop((*self)->exec);
        })
    }
}
