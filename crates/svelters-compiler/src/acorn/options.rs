//! Port of acorn 8.19.0 `src/options.js`.
//!
//! SVELTERS: only the options that influence parsing are kept. `locations`, `ranges`,
//! `onToken`, `onInsertedSemicolon`, `onTrailingComma`, `program`, `sourceFile` and
//! `directSourceFile` are out of scope (byte offsets only; comments go to a `Vec`).

/// acorn `sourceType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcornSourceType {
    Script,
    Module,
    CommonJs,
}

/// acorn `allowReserved` (`null` = `None` in `AcornOptions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllowReserved {
    Yes,
    No,
    Never,
}

/// `ecmaVersion: "latest"`.
pub const ECMA_LATEST: u32 = 100_000_000;

/// The full set of acorn options this port supports (see `defaultOptions` for documentation).
#[derive(Clone, Debug)]
pub struct AcornOptions {
    /// `ecmaVersion`: 3, 5, 6..=17, 2015..=2026 or `ECMA_LATEST`.
    pub ecma_version: u32,
    pub source_type: AcornSourceType,
    /// When set to true, enable strict parsing mode even if `sourceType` is `"script"`.
    pub strict: bool,
    /// By default, reserved words are only enforced if ecmaVersion >= 5.
    pub allow_reserved: Option<AllowReserved>,
    pub allow_return_outside_function: bool,
    pub allow_import_export_everywhere: bool,
    pub allow_await_outside_function: Option<bool>,
    pub allow_super_outside_method: Option<bool>,
    /// Defaults to `ecmaVersion >= 14`.
    pub allow_hash_bang: Option<bool>,
    pub check_private_fields: bool,
    pub preserve_parens: bool,
    /// Svelte's `<script>` hack (not an acorn option): `undefinedExports` is cleared after
    /// every `parseStatement`.
    pub allow_undefined_exports: bool,
}

impl Default for AcornOptions {
    fn default() -> Self {
        AcornOptions {
            ecma_version: 16,
            source_type: AcornSourceType::Script,
            strict: false,
            allow_reserved: None,
            allow_return_outside_function: false,
            allow_import_export_everywhere: false,
            allow_await_outside_function: None,
            allow_super_outside_method: None,
            allow_hash_bang: None,
            check_private_fields: true,
            preserve_parens: false,
            allow_undefined_exports: false,
        }
    }
}

/// Options after `getOptions` normalization.
#[derive(Clone, Debug)]
pub struct NormalizedOptions {
    pub ecma_version: u32,
    pub source_type: AcornSourceType,
    pub strict: bool,
    pub allow_reserved: AllowReserved,
    pub allow_return_outside_function: bool,
    pub allow_import_export_everywhere: bool,
    pub allow_await_outside_function: bool,
    pub allow_super_outside_method: bool,
    pub allow_hash_bang: bool,
    pub check_private_fields: bool,
    pub preserve_parens: bool,
    pub allow_undefined_exports: bool,
}

// Interpret and default an options object

pub fn get_options(opts: &AcornOptions) -> Result<NormalizedOptions, String> {
    let mut ecma_version = opts.ecma_version;
    if ecma_version >= 2015 && ecma_version != ECMA_LATEST {
        ecma_version -= 2009;
    }
    let allow_reserved = opts.allow_reserved.unwrap_or(if ecma_version < 5 { AllowReserved::Yes } else { AllowReserved::No });
    let allow_hash_bang = opts.allow_hash_bang.unwrap_or(ecma_version >= 14);
    if opts.source_type == AcornSourceType::CommonJs && opts.allow_await_outside_function == Some(true) {
        return Err("Cannot use allowAwaitOutsideFunction with sourceType: commonjs".into());
    }
    Ok(NormalizedOptions {
        ecma_version,
        source_type: opts.source_type,
        strict: opts.strict,
        allow_reserved,
        allow_return_outside_function: opts.allow_return_outside_function,
        allow_import_export_everywhere: opts.allow_import_export_everywhere,
        allow_await_outside_function: opts.allow_await_outside_function.unwrap_or(false),
        allow_super_outside_method: opts.allow_super_outside_method.unwrap_or(false),
        allow_hash_bang,
        check_private_fields: opts.check_private_fields,
        preserve_parens: opts.preserve_parens,
        allow_undefined_exports: opts.allow_undefined_exports,
    })
}
