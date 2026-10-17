//! Port of acorn 8.19.0 `src/scopeflags.js`.

// Each scope gets a bitset that may contain these flags
pub const SCOPE_TOP: u32 = 1;
pub const SCOPE_FUNCTION: u32 = 2;
pub const SCOPE_ASYNC: u32 = 4;
pub const SCOPE_GENERATOR: u32 = 8;
pub const SCOPE_ARROW: u32 = 16;
pub const SCOPE_SIMPLE_CATCH: u32 = 32;
pub const SCOPE_SUPER: u32 = 64;
pub const SCOPE_DIRECT_SUPER: u32 = 128;
pub const SCOPE_CLASS_STATIC_BLOCK: u32 = 256;
pub const SCOPE_CLASS_FIELD_INIT: u32 = 512;
pub const SCOPE_SWITCH: u32 = 1024;
pub const SCOPE_VAR: u32 = SCOPE_TOP | SCOPE_FUNCTION | SCOPE_CLASS_STATIC_BLOCK;

pub fn function_flags(is_async: bool, generator: bool) -> u32 {
    SCOPE_FUNCTION | if is_async { SCOPE_ASYNC } else { 0 } | if generator { SCOPE_GENERATOR } else { 0 }
}

// Used in checkLVal* and declareName to determine the type of a binding
/// Not a binding
pub const BIND_NONE: u32 = 0;
/// Var-style binding
pub const BIND_VAR: u32 = 1;
/// Let- or const-style binding
pub const BIND_LEXICAL: u32 = 2;
/// Function declaration
pub const BIND_FUNCTION: u32 = 3;
/// Simple (identifier pattern) catch binding
pub const BIND_SIMPLE_CATCH: u32 = 4;
/// Special case for function names as bound inside the function
pub const BIND_OUTSIDE: u32 = 5;
