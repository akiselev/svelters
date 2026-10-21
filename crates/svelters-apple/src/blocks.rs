//! Completion blocks (NON-GUI-TYPES §2.2 Apple: "blocks → CALLBACK").
//!
//! An Objective-C block has a C signature fixed at compile time, and this crate has no libffi, so the
//! core builds completion blocks of a small set of **shapes**: one letter per block argument, every
//! block returning `void`.
//!
//! | letter | ObjC argument | delivered as |
//! |---|---|---|
//! | `o` | any object pointer | [`BlockArg::Obj`] |
//! | `e` | `NSError *` | [`BlockArg::Obj`] (the caller turns a non-nil one into a rejection) |
//! | `b` | `BOOL` | [`BlockArg::Bool`] |
//! | `i` | `NSInteger` / `NSUInteger` / an enum | [`BlockArg::Int`] |
//! | `d` | `CGFloat` / `double` | [`BlockArg::F64`] |
//!
//! The classifier (`tools/metadata/appkit/types_appkit_classify.py`, `BLOCK_SHAPES`) binds an async
//! member only when its completion block has one of [`SHAPES`]; anything else stays blocked with its
//! reason in the facts. A test checks the two lists agree through the generated tables.
//!
//! Blocks may run on any thread (NSItemProvider, NSFileManager and NSURLSession complete on their own
//! queues). The block itself only retains its object arguments and hands them to `deliver`, which must
//! be thread-safe; converting them to ABI values is the caller's job, on the main thread.

/// The completion-block shapes the core builds.
pub const SHAPES: &[&str] = &["", "e", "o", "oe", "b", "be", "i", "ie", "d"];

/// Whether [`SHAPES`] has `shape`.
pub fn supported(shape: &str) -> bool {
    SHAPES.contains(&shape)
}

/// Which argument (if any) carries the `NSError *`, and which carries the result.
pub fn roles(shape: &str) -> (Option<usize>, Option<usize>) {
    let err = shape.find('e');
    let res = shape.char_indices().find(|(_, c)| *c != 'e').map(|(i, _)| i);
    (err, res)
}

#[cfg(target_vendor = "apple")]
pub use apple::*;

#[cfg(target_vendor = "apple")]
mod apple {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use std::sync::Arc;

    /// One argument a completion block received.
    pub enum BlockArg {
        Obj(Option<Retained<AnyObject>>),
        Bool(bool),
        Int(i64),
        F64(f64),
    }

    /// The arguments of one block invocation, moved to the main thread for conversion.
    pub struct Args(pub Vec<BlockArg>);
    // SAFETY: the objects are retained here and only touched again on the main thread (the receiver
    // converts them there); ObjC retain/release are thread-safe.
    unsafe impl Send for Args {}

    /// What a block calls with its arguments; runs on whatever thread the block runs on.
    pub type Deliver = Arc<dyn Fn(Args) + Send + Sync>;

    /// A built block: keeps it alive while the native call runs (the callee copies it if it keeps it).
    pub struct Built {
        ptr: *mut std::ffi::c_void,
        _keep: Box<dyn std::any::Any>,
    }

    impl Built {
        /// The block as an object pointer (`@?` argument).
        pub fn as_ptr(&self) -> *mut std::ffi::c_void {
            self.ptr
        }
    }

    fn obj(p: *mut AnyObject) -> BlockArg {
        // SAFETY: a block argument object is valid for the call; retain it for the hop.
        BlockArg::Obj(unsafe { Retained::retain(p) })
    }

    fn built<F: ?Sized + 'static>(b: RcBlock<F>) -> Built {
        Built { ptr: RcBlock::as_ptr(&b) as *mut std::ffi::c_void, _keep: Box::new(b) }
    }

    /// Builds a completion block of `shape` that calls `deliver` with its arguments.
    pub fn completion(shape: &str, deliver: Deliver) -> Result<Built, String> {
        let d = deliver;
        Ok(match shape {
            "" => built(RcBlock::new(move || d(Args(vec![])))),
            "e" | "o" => built(RcBlock::new(move |a: *mut AnyObject| d(Args(vec![obj(a)])))),
            "oe" => built(RcBlock::new(move |a: *mut AnyObject, e: *mut AnyObject| d(Args(vec![obj(a), obj(e)])))),
            "b" => built(RcBlock::new(move |a: Bool| d(Args(vec![BlockArg::Bool(a.as_bool())])))),
            "be" => built(RcBlock::new(move |a: Bool, e: *mut AnyObject| d(Args(vec![BlockArg::Bool(a.as_bool()), obj(e)])))),
            "i" => built(RcBlock::new(move |a: isize| d(Args(vec![BlockArg::Int(a as i64)])))),
            "ie" => built(RcBlock::new(move |a: isize, e: *mut AnyObject| d(Args(vec![BlockArg::Int(a as i64), obj(e)])))),
            "d" => built(RcBlock::new(move |a: f64| d(Args(vec![BlockArg::F64(a)])))),
            other => return Err(format!("completion blocks of shape ({other}) are not supported")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_of_shapes() {
        assert_eq!(roles("oe"), (Some(1), Some(0)));
        assert_eq!(roles("e"), (Some(0), None));
        assert_eq!(roles(""), (None, None));
        assert!(supported("oe") && !supported("oo"));
    }
}
