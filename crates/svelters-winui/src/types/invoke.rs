//! The dynamic WinRT invoker: calls vtable slot `n` of an interface pointer with arguments known only at run time.
//!
//! Every WinRT method has the shape `HRESULT method(this, args..., [out, retval] T*)` with the `__stdcall` calling
//! convention, which on x86_64 is the Microsoft x64 convention (`extern "win64"`, what `extern "system"` means on
//! Windows). There, each argument takes one 8-byte slot: the first four (`this` and three more) go in RCX/RDX/R8/R9,
//! or in XMM1-XMM3 when they are floating point; the rest go on the stack, a float as its bits. Structs of 1, 2, 4 or
//! 8 bytes pass as an integer of that size, larger ones by pointer to a caller-owned copy. The caller reserves the
//! stack and cleans it up, so passing more arguments than the callee reads is harmless.
//!
//! So one call shape per float pattern of slots 1-3 (3^3 = 27 function types, each with 15 argument slots after
//! `this`) covers every WinRT method with up to 15 ABI parameters: no per-method native code. The same convention is
//! available on any x86_64 target as `extern "win64"`, so the invoker and the delegate thunks are unit-tested on Linux.
//!
//! Other architectures (aarch64 Windows) are refused with an error for now.
use std::ffi::c_void;

/// One ABI argument slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Word {
    /// Integers, enums, booleans, pointers, HSTRINGs and structs of 1/2/4/8 bytes (their bits).
    I(u64),
    F32(f32),
    F64(f64),
}

impl Word {
    pub fn bits(self) -> u64 {
        match self {
            Word::I(v) => v,
            Word::F32(f) => f.to_bits() as u64,
            Word::F64(f) => f.to_bits(),
        }
    }

    pub fn ptr<T>(p: *const T) -> Word {
        Word::I(p as usize as u64)
    }
}

/// Argument slots after `this`.
pub const MAX_WORDS: usize = 15;

trait FromWord: Copy {
    fn from_word(w: Word) -> Self;
}

impl FromWord for u64 {
    fn from_word(w: Word) -> u64 {
        w.bits()
    }
}

impl FromWord for f32 {
    fn from_word(w: Word) -> f32 {
        match w {
            Word::F32(f) => f,
            Word::F64(f) => f as f32,
            Word::I(v) => f32::from_bits(v as u32),
        }
    }
}

impl FromWord for f64 {
    fn from_word(w: Word) -> f64 {
        match w {
            Word::F64(f) => f,
            Word::F32(f) => f as f64,
            Word::I(v) => f64::from_bits(v),
        }
    }
}

#[cfg(target_arch = "x86_64")]
macro_rules! shape {
    ($f:expr, $this:expr, $w:expr; $a:ty, $b:ty, $c:ty) => {{
        type F = unsafe extern "win64" fn(*mut c_void, $a, $b, $c, u64, u64, u64, u64, u64, u64, u64, u64, u64, u64, u64, u64) -> i32;
        // SAFETY: the caller guarantees `$f` is a function with the Microsoft x64 convention whose first four
        // parameters have these register classes; extra slots are ignored by the callee.
        let f: F = unsafe { std::mem::transmute::<*const c_void, F>($f) };
        let w = $w;
        unsafe {
            f(
                $this,
                <$a as FromWord>::from_word(w[0]),
                <$b as FromWord>::from_word(w[1]),
                <$c as FromWord>::from_word(w[2]),
                w[3].bits(),
                w[4].bits(),
                w[5].bits(),
                w[6].bits(),
                w[7].bits(),
                w[8].bits(),
                w[9].bits(),
                w[10].bits(),
                w[11].bits(),
                w[12].bits(),
                w[13].bits(),
                w[14].bits(),
            )
        }
    }};
}

/// Calls `f(this, words...)` (Microsoft x64 convention) and returns its HRESULT.
///
/// # Safety
/// `f` must be a function pointer with the WinRT ABI whose parameters after `this` are exactly the slots `words`
/// describes (in count, order and register class), and every pointer in `words` must be valid for the call.
#[cfg(target_arch = "x86_64")]
pub unsafe fn call(f: *const c_void, this: *mut c_void, words: &[Word]) -> Result<i32, String> {
    if words.len() > MAX_WORDS {
        return Err(format!("{} ABI arguments: the dynamic invoker takes at most {MAX_WORDS}", words.len()));
    }
    let mut w = [Word::I(0); MAX_WORDS];
    w[..words.len()].copy_from_slice(words);
    let class = |x: Word| match x {
        Word::I(_) => 0u8,
        Word::F32(_) => 1,
        Word::F64(_) => 2,
    };
    let hr = match (class(w[0]), class(w[1]), class(w[2])) {
        (0, 0, 0) => shape!(f, this, w; u64, u64, u64),
        (0, 0, 1) => shape!(f, this, w; u64, u64, f32),
        (0, 0, 2) => shape!(f, this, w; u64, u64, f64),
        (0, 1, 0) => shape!(f, this, w; u64, f32, u64),
        (0, 1, 1) => shape!(f, this, w; u64, f32, f32),
        (0, 1, 2) => shape!(f, this, w; u64, f32, f64),
        (0, 2, 0) => shape!(f, this, w; u64, f64, u64),
        (0, 2, 1) => shape!(f, this, w; u64, f64, f32),
        (0, 2, 2) => shape!(f, this, w; u64, f64, f64),
        (1, 0, 0) => shape!(f, this, w; f32, u64, u64),
        (1, 0, 1) => shape!(f, this, w; f32, u64, f32),
        (1, 0, 2) => shape!(f, this, w; f32, u64, f64),
        (1, 1, 0) => shape!(f, this, w; f32, f32, u64),
        (1, 1, 1) => shape!(f, this, w; f32, f32, f32),
        (1, 1, 2) => shape!(f, this, w; f32, f32, f64),
        (1, 2, 0) => shape!(f, this, w; f32, f64, u64),
        (1, 2, 1) => shape!(f, this, w; f32, f64, f32),
        (1, 2, 2) => shape!(f, this, w; f32, f64, f64),
        (2, 0, 0) => shape!(f, this, w; f64, u64, u64),
        (2, 0, 1) => shape!(f, this, w; f64, u64, f32),
        (2, 0, 2) => shape!(f, this, w; f64, u64, f64),
        (2, 1, 0) => shape!(f, this, w; f64, f32, u64),
        (2, 1, 1) => shape!(f, this, w; f64, f32, f32),
        (2, 1, 2) => shape!(f, this, w; f64, f32, f64),
        (2, 2, 0) => shape!(f, this, w; f64, f64, u64),
        (2, 2, 1) => shape!(f, this, w; f64, f64, f32),
        _ => shape!(f, this, w; f64, f64, f64),
    };
    Ok(hr)
}

#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn call(_f: *const c_void, _this: *mut c_void, _words: &[Word]) -> Result<i32, String> {
    Err("dynamic WinRT calls are implemented for x86_64 only (NON-GUI types, N4)".into())
}

/// Calls vtable slot `slot` of the interface pointer `this`.
///
/// # Safety
/// `this` must be a live COM interface pointer whose vtable has at least `slot + 1` entries, and the slot's ABI
/// signature must match `words` (see [`call`]).
pub unsafe fn call_slot(this: *mut c_void, slot: u16, words: &[Word]) -> Result<i32, String> {
    if this.is_null() {
        return Err("null interface pointer".into());
    }
    // SAFETY: a COM interface pointer points at its vtable pointer.
    let f = unsafe {
        let vtbl = *(this as *const *const *const c_void);
        *vtbl.add(slot as usize)
    };
    unsafe { call(f, this, words) }
}

// ---- callee side: delegate Invoke thunks -----------------------------------------------------------------------

/// What a delegate thunk forwards to: the delegate object and its integer-class arguments.
pub type Forward = unsafe fn(this: *mut c_void, args: &[u64]) -> i32;

/// The most `Invoke` arguments a dynamic delegate takes (all integer-class; float arguments are refused when the
/// facts are made).
pub const MAX_DELEGATE_ARGS: usize = 6;

/// Builds the `Invoke` thunk for `n` integer-class arguments that calls `$fwd(this, &[args])`.
#[macro_export]
#[doc(hidden)]
macro_rules! __svl_delegate_thunks {
    ($fwd:path) => {
        [
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t0(this: *mut ::std::ffi::c_void) -> i32 {
                    unsafe { $fwd(this, &[]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t0(this: *mut ::std::ffi::c_void) -> i32 {
                    unsafe { $fwd(this, &[]) }
                }
                t0 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t1(this: *mut ::std::ffi::c_void, a: u64) -> i32 {
                    unsafe { $fwd(this, &[a]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t1(this: *mut ::std::ffi::c_void, a: u64) -> i32 {
                    unsafe { $fwd(this, &[a]) }
                }
                t1 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t2(this: *mut ::std::ffi::c_void, a: u64, b: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t2(this: *mut ::std::ffi::c_void, a: u64, b: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b]) }
                }
                t2 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t3(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t3(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c]) }
                }
                t3 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t4(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t4(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d]) }
                }
                t4 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t5(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64, e: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d, e]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t5(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64, e: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d, e]) }
                }
                t5 as *const ::std::ffi::c_void
            },
            {
                #[cfg(target_arch = "x86_64")]
                unsafe extern "win64" fn t6(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d, e, f]) }
                }
                #[cfg(not(target_arch = "x86_64"))]
                unsafe extern "C" fn t6(this: *mut ::std::ffi::c_void, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> i32 {
                    unsafe { $fwd(this, &[a, b, c, d, e, f]) }
                }
                t6 as *const ::std::ffi::c_void
            },
        ]
    };
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn seen(s: String) {
        SEEN.with(|v| v.borrow_mut().push(s));
    }

    fn take() -> Vec<String> {
        SEEN.with(|v| std::mem::take(&mut *v.borrow_mut()))
    }

    // Callees with the shapes WinRT methods have: mixed register classes, stack arguments, an out pointer.
    unsafe extern "win64" fn ints(this: *mut c_void, a: u32, b: i64, out: *mut i32) -> i32 {
        seen(format!("ints {} {a} {b}", this as usize));
        unsafe { *out = a as i32 + b as i32 };
        0
    }

    unsafe extern "win64" fn floats(_this: *mut c_void, a: f32, b: u64, c: f64, d: f32, e: f64, f: u8, out: *mut f64) -> i32 {
        seen(format!("floats {a} {b} {c} {d} {e} {f}"));
        unsafe { *out = a as f64 + c + d as f64 + e };
        -2147024809 // E_INVALIDARG: the HRESULT comes back as is
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Thickness {
        l: f64,
        t: f64,
        r: f64,
        b: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Point {
        x: f32,
        y: f32,
    }

    unsafe extern "win64" fn structs(_this: *mut c_void, t: Thickness, p: Point, ok: bool, out: *mut Thickness) -> i32 {
        seen(format!("structs {t:?} {p:?} {ok}"));
        unsafe { *out = Thickness { l: t.l + p.x as f64, t: t.t + p.y as f64, r: t.r, b: t.b } };
        0
    }

    #[test]
    fn integer_arguments_and_out_pointer() {
        let mut out = 0i32;
        let hr = unsafe { call(ints as *const c_void, 7 as *mut c_void, &[Word::I(40), Word::I(2), Word::ptr(&mut out)]) }.unwrap();
        assert_eq!(hr, 0);
        assert_eq!(out, 42);
        assert_eq!(take(), ["ints 7 40 2"]);
    }

    #[test]
    fn float_registers_and_stack_slots() {
        let mut out = 0f64;
        let w = [Word::F32(1.5), Word::I(9), Word::F64(2.25), Word::F32(0.5), Word::F64(4.0), Word::I(255), Word::ptr(&mut out)];
        let hr = unsafe { call(floats as *const c_void, std::ptr::null_mut(), &w) }.unwrap();
        assert_eq!(hr, -2147024809);
        assert_eq!(out, 8.25);
        assert_eq!(take(), ["floats 1.5 9 2.25 0.5 4 255"]);
    }

    #[test]
    fn structs_by_pointer_and_by_value() {
        // A 32-byte struct goes by pointer to a copy; an 8-byte one in an integer slot (its bits).
        let t = Thickness { l: 1.0, t: 2.0, r: 3.0, b: 4.0 };
        let p = Point { x: 10.0, y: 20.0 };
        let pbits = u64::from_le_bytes(unsafe { std::mem::transmute::<Point, [u8; 8]>(p) });
        let mut out = Thickness { l: 0.0, t: 0.0, r: 0.0, b: 0.0 };
        let w = [Word::ptr(&t), Word::I(pbits), Word::I(1), Word::ptr(&mut out)];
        unsafe { call(structs as *const c_void, std::ptr::null_mut(), &w) }.unwrap();
        assert_eq!(out, Thickness { l: 11.0, t: 22.0, r: 3.0, b: 4.0 });
        assert_eq!(take().len(), 1);
    }

    #[test]
    fn vtable_slot_dispatch() {
        #[repr(C)]
        struct Obj {
            vtbl: *const [*const c_void; 8],
        }
        let vtbl: [*const c_void; 8] = [std::ptr::null(); 8];
        let mut v = vtbl;
        v[7] = ints as *const c_void;
        let obj = Obj { vtbl: &v };
        let mut out = 0i32;
        let this = &obj as *const Obj as *mut c_void;
        unsafe { call_slot(this, 7, &[Word::I(1), Word::I(2), Word::ptr(&mut out)]) }.unwrap();
        assert_eq!(out, 3);
        take();
        assert!(unsafe { call(ints as *const c_void, this, &[Word::I(0); 16]) }.is_err());
    }

    unsafe fn record(this: *mut c_void, args: &[u64]) -> i32 {
        seen(format!("{} {args:?}", this as usize));
        args.len() as i32
    }

    #[test]
    fn delegate_thunks_forward_their_arguments() {
        let thunks = crate::__svl_delegate_thunks!(record);
        type T2 = unsafe extern "win64" fn(*mut c_void, u64, u64) -> i32;
        let t2: T2 = unsafe { std::mem::transmute::<*const c_void, T2>(thunks[2]) };
        assert_eq!(unsafe { t2(5 as *mut c_void, 11, 12) }, 2);
        // A thunk reached through the invoker, as WinRT would call it.
        assert_eq!(unsafe { call(thunks[3], 6 as *mut c_void, &[Word::I(1), Word::I(2), Word::I(3)]) }.unwrap(), 3);
        assert_eq!(take(), ["5 [11, 12]", "6 [1, 2, 3]"]);
    }
}
