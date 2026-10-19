//! See Cargo.toml. The loop half of an Android backend, written against the raw NDK C API:
//! wake = eventfd on the main ALooper, schedule_wake = timerfd on the same looper,
//! request_frame = AChoreographer_postFrameCallback64, and one JNI entry point that a thin
//! Activity calls from onCreate (on the main thread). Not run anywhere.
#![cfg(target_os = "android")]
use std::ffi::c_void;
use std::os::raw::c_int;
use std::ptr;

use jni_sys::{jclass, jlong, jobject, JNIEnv, JavaVM, JNI_VERSION_1_6, jint};
use ndk_sys::{
    ALooper, ALooper_addFd, ALooper_forThread, ALooper_acquire, AChoreographer, AChoreographer_getInstance,
    AChoreographer_postFrameCallback64, ALOOPER_EVENT_INPUT,
};

struct Loop {
    looper: *mut ALooper,
    wake_fd: c_int,
    timer_fd: c_int,
    choreo: *mut AChoreographer,
}

static mut LOOP: Option<Loop> = None;

unsafe extern "C" fn on_wake(fd: c_int, _events: c_int, _data: *mut c_void) -> c_int {
    let mut n: u64 = 0;
    unsafe { libc::read(fd, &mut n as *mut u64 as *mut c_void, 8) }; // drain: wakes coalesce
    // host.checkpoint()
    1 // keep the callback registered
}

unsafe extern "C" fn on_timer(fd: c_int, _events: c_int, _data: *mut c_void) -> c_int {
    let mut n: u64 = 0;
    unsafe { libc::read(fd, &mut n as *mut u64 as *mut c_void, 8) };
    // host.checkpoint() (timer due)
    1
}

unsafe extern "C" fn on_frame(_frame_time_nanos: i64, _data: *mut c_void) {
    // host frame callback; re-post only if JS asked for another frame
}

/// Any thread: schedule a checkpoint (svl_backend.wake).
pub fn wake() {
    let one: u64 = 1;
    #[allow(static_mut_refs)]
    if let Some(l) = unsafe { LOOP.as_ref() } {
        unsafe { libc::write(l.wake_fd, &one as *const u64 as *const c_void, 8) };
    }
}

/// Main thread: re-arm the single timer (svl_backend.schedule_wake); ms < 0 cancels.
pub fn schedule_wake(ms: i64) {
    #[allow(static_mut_refs)]
    let Some(l) = (unsafe { LOOP.as_ref() }) else { return };
    let ms = ms.max(0);
    let spec = libc::itimerspec {
        it_interval: libc::timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: libc::timespec { tv_sec: (ms / 1000) as _, tv_nsec: ((ms % 1000) * 1_000_000 + if ms == 0 { 1 } else { 0 }) as _ },
    };
    unsafe { libc::timerfd_settime(l.timer_fd, 0, &spec, ptr::null_mut()) };
}

/// Main thread: request one frame callback (svl_backend.request_frame). API 29+.
pub fn request_frame() {
    #[allow(static_mut_refs)]
    if let Some(l) = unsafe { LOOP.as_ref() } {
        unsafe { AChoreographer_postFrameCallback64(l.choreo, Some(on_frame), ptr::null_mut()) };
    }
}

/// `static native void nativeStart(Activity a, FrameLayout root)` on the helper class, called from
/// Activity.onCreate on the main thread. Attaches the loop sources to the main looper.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_svelters_SveltersActivity_nativeStart(_env: *mut JNIEnv, _cls: jclass, _activity: jobject, _root: jobject) -> jlong {
    unsafe {
        let looper = ALooper_forThread(); // the main thread's looper (Looper.prepareMainLooper ran)
        ALooper_acquire(looper);
        let wake_fd = libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC);
        let timer_fd = libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_NONBLOCK | libc::TFD_CLOEXEC);
        ALooper_addFd(looper, wake_fd, 0, ALOOPER_EVENT_INPUT as c_int, Some(on_wake), ptr::null_mut());
        ALooper_addFd(looper, timer_fd, 0, ALOOPER_EVENT_INPUT as c_int, Some(on_timer), ptr::null_mut());
        let choreo = AChoreographer_getInstance();
        LOOP = Some(Loop { looper, wake_fd, timer_fd, choreo });
    }
    1
}

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(_vm: *mut JavaVM, _reserved: *mut c_void) -> jint {
    JNI_VERSION_1_6
}
