//! Port of acorn 8.19.0 `src/regexp.js` (https://github.com/acornjs/acorn tag 8.19.0, MIT,
//! Marijn Haverbeke and contributors); `src/unicode-property-data.js` is in
//! `regexp/unicode_property_data.rs`. Owner: agent B.
//!
//! Upstream these are `Parser` methods (`pp.validateRegExpFlags`, `pp.validateRegExpPattern`,
//! `pp.regexp_*`) that `this.raise(...)`; here `pp` is [`Pp`] (it only needs `ecmaVersion`) and
//! raising is `Err(message)` propagated with `?`. The validator works on the pattern's UTF-16
//! code units exactly like acorn (`charCodeAt`, surrogate pairs combined only in `u`/`v` mode
//! or where acorn forces it).
//!
//! Error position: acorn raises every regexp error (flags and pattern) at `state.start`, which
//! `readRegexp` sets to the position of the pattern's first character (just after the opening
//! `/`). So the offset returned by [`validate_pattern`] is always 0 (relative to the pattern
//! start); the caller adds the pattern's absolute byte offset. [`validate_flags`] raises at the
//! same position.

mod unicode_property_data;

use std::collections::HashMap;

use unicode_property_data::{UnicodePropertyData, unicode_property_values};

/// acorn `validateRegExpFlags`: Err(message) uses acorn's wording ("Invalid regular expression
/// flag", "Duplicate regular expression flag"). acorn raises it at the pattern start (see the
/// module docs). `ecma_version` is acorn's normalized `ecmaVersion` (2015 and later years are
/// accepted too and normalized like acorn's `getOptions`).
pub fn validate_flags(flags: &str, ecma_version: u32) -> Result<(), String> {
    let pp = Pp::new(ecma_version);
    let mut state = RegExpValidationState::new(&pp);
    state.reset(0, "", flags);
    pp.validate_reg_exp_flags(&state)
}

/// acorn `validateRegExpPattern`: Err((message, offset)) with acorn's wording, e.g.
/// "Invalid regular expression: /(/: Unterminated group". The offset is where acorn raises,
/// relative to the first byte of `pattern`: always 0, because acorn raises at `state.start`
/// (the pattern start), never at the offending character. `flags` must already have passed
/// [`validate_flags`] (acorn validates the flags first).
pub fn validate_pattern(
    pattern: &str,
    flags: &str,
    ecma_version: u32,
) -> Result<(), (String, u32)> {
    let pp = Pp::new(ecma_version);
    let mut state = RegExpValidationState::new(&pp);
    state.reset(0, pattern, flags);
    pp.validate_reg_exp_pattern(&mut state)
        .map_err(|message| (message, state.start))
}

/// A raised error: the full message of acorn's `SyntaxError` (without the ` (line:col)` suffix).
type Raise<T> = Result<T, String>;

/// The part of acorn's `Parser` that the regexp validator uses (`this.options.ecmaVersion`).
struct Pp {
    ecma_version: u32,
}

impl Pp {
    fn new(ecma_version: u32) -> Self {
        // SVELTERS: acorn's getOptions: `if (options.ecmaVersion >= 2015) options.ecmaVersion -= 2009`.
        let ecma_version = if ecma_version >= 2015 {
            ecma_version - 2009
        } else {
            ecma_version
        };
        Pp { ecma_version }
    }
}

// Track disjunction structure to determine whether a duplicate
// capture group name is allowed because it is in a separate branch.
// SVELTERS: BranchIDs live in an arena (`RegExpValidationState::branch_ids`) and are compared by
// index, which is acorn's object identity.
#[derive(Clone, Copy)]
struct BranchId {
    // Parent disjunction branch
    parent: Option<usize>,
    // Identifies this set of sibling branches
    base: usize,
}

impl RegExpValidationState<'_> {
    /// `new BranchID(parent, base)`: `base || this`.
    fn new_branch_id(&mut self, parent: Option<usize>, base: Option<usize>) -> usize {
        let id = self.branch_ids.len();
        self.branch_ids.push(BranchId {
            parent,
            base: base.unwrap_or(id),
        });
        id
    }

    /// `this.separatedFrom(alt)`
    fn separated_from(&self, this: usize, alt: usize) -> bool {
        // A branch is separate from another branch if they or any of
        // their parents are siblings in a given disjunction
        let mut self_ = Some(this);
        while let Some(s) = self_ {
            let mut other = Some(alt);
            while let Some(o) = other {
                if self.branch_ids[s].base == self.branch_ids[o].base && s != o {
                    return true;
                }
                other = self.branch_ids[o].parent;
            }
            self_ = self.branch_ids[s].parent;
        }
        false
    }

    /// `this.sibling()`
    fn sibling(&mut self, this: usize) -> usize {
        let BranchId { parent, base } = self.branch_ids[this];
        self.new_branch_id(parent, Some(base))
    }
}

/// `state.groupNames[name]`: acorn stores `true` (ecmaVersion < 16) or an array of BranchIDs
/// (ecmaVersion >= 16). SVELTERS: both are a Vec (empty for `true`); presence is truthiness.
type GroupNames = HashMap<String, Vec<Option<usize>>>;

pub struct RegExpValidationState<'p> {
    parser: &'p Pp,
    valid_flags: String,
    unicode_properties: Option<&'static UnicodePropertyData>,
    /// The pattern as given (for messages).
    source_str: String,
    /// The pattern as UTF-16 code units (acorn's `this.source`).
    source: Vec<u16>,
    flags: String,
    start: u32,
    switch_u: bool,
    switch_v: bool,
    switch_n: bool,
    pos: usize,
    /// JS number: kept as f64 so that huge decimal/hex escapes behave like acorn.
    last_int_value: f64,
    last_string_value: String,
    last_assertion_is_quantifiable: bool,
    num_capturing_parens: f64,
    max_back_reference: f64,
    group_names: GroupNames,
    back_reference_names: Vec<String>,
    branch_id: Option<usize>,
    branch_ids: Vec<BranchId>,
}

impl<'p> RegExpValidationState<'p> {
    fn new(parser: &'p Pp) -> Self {
        let ev = parser.ecma_version;
        RegExpValidationState {
            parser,
            valid_flags: format!(
                "gim{}{}{}{}",
                if ev >= 6 { "uy" } else { "" },
                if ev >= 9 { "s" } else { "" },
                if ev >= 13 { "d" } else { "" },
                if ev >= 15 { "v" } else { "" }
            ),
            unicode_properties: unicode_property_values(if ev >= 14 { 14 } else { ev }),
            source_str: String::new(),
            source: Vec::new(),
            flags: String::new(),
            start: 0,
            switch_u: false,
            switch_v: false,
            switch_n: false,
            pos: 0,
            last_int_value: 0.0,
            last_string_value: String::new(),
            last_assertion_is_quantifiable: false,
            num_capturing_parens: 0.0,
            max_back_reference: 0.0,
            group_names: HashMap::new(),
            back_reference_names: Vec::new(),
            branch_id: None,
            branch_ids: Vec::new(),
        }
    }

    fn reset(&mut self, start: u32, pattern: &str, flags: &str) {
        let unicode_sets = flags.contains('v');
        let unicode = flags.contains('u');
        self.start = start;
        self.source_str = pattern.to_owned();
        self.source = pattern.encode_utf16().collect();
        self.flags = flags.to_owned();
        if unicode_sets && self.parser.ecma_version >= 15 {
            self.switch_u = true;
            self.switch_v = true;
            self.switch_n = true;
        } else {
            self.switch_u = unicode && self.parser.ecma_version >= 6;
            self.switch_v = false;
            self.switch_n = unicode && self.parser.ecma_version >= 9;
        }
    }

    fn raise(&self, message: &str) -> String {
        format!(
            "Invalid regular expression: /{}/: {}",
            self.source_str, message
        )
    }

    // If u flag is given, this returns the code point at the index (it combines a surrogate pair).
    // Otherwise, this returns the code unit of the index (can be a part of a surrogate pair).
    fn at(&self, i: usize, force_u: bool) -> i32 {
        let s = &self.source;
        let l = s.len();
        if i >= l {
            return -1;
        }
        let c = s[i] as i32;
        if !(force_u || self.switch_u) || c <= 0xD7FF || c >= 0xE000 || i + 1 >= l {
            return c;
        }
        let next = s[i + 1] as i32;
        if (0xDC00..=0xDFFF).contains(&next) {
            (c << 10) + next - 0x35FDC00
        } else {
            c
        }
    }

    fn next_index(&self, i: usize, force_u: bool) -> usize {
        let s = &self.source;
        let l = s.len();
        if i >= l {
            return l;
        }
        let c = s[i] as i32;
        if !(force_u || self.switch_u) || c <= 0xD7FF || c >= 0xE000 || i + 1 >= l || {
            let next = s[i + 1] as i32;
            !(0xDC00..=0xDFFF).contains(&next)
        } {
            return i + 1;
        }
        i + 2
    }

    fn current(&self) -> i32 {
        self.at(self.pos, false)
    }

    fn current_u(&self, force_u: bool) -> i32 {
        self.at(self.pos, force_u)
    }

    fn lookahead(&self) -> i32 {
        self.at(self.next_index(self.pos, false), false)
    }

    fn advance(&mut self) {
        self.advance_u(false)
    }

    fn advance_u(&mut self, force_u: bool) {
        self.pos = self.next_index(self.pos, force_u);
    }

    fn eat(&mut self, ch: i32) -> bool {
        if self.current() == ch {
            self.advance();
            return true;
        }
        false
    }

    fn eat_chars(&mut self, chs: &[i32]) -> bool {
        let mut pos = self.pos;
        for &ch in chs {
            let current = self.at(pos, false);
            if current == -1 || current != ch {
                return false;
            }
            pos = self.next_index(pos, false);
        }
        self.pos = pos;
        true
    }
}

/// `codePointToString(code)` appended to a Rust string. Callers only pass valid scalar values
/// (identifier characters, ASCII), so no lone surrogate can reach a `String`.
fn push_code_point(s: &mut String, code: f64) {
    if let Some(c) = char::from_u32(code as u32) {
        s.push(c);
    }
}

// Return values used by character set parsing methods, needed to
// forbid negation of sets that can match strings.
type CharSet = u8;
const CHAR_SET_NONE: CharSet = 0; // Nothing parsed
const CHAR_SET_OK: CharSet = 1; // Construct parsed, cannot contain strings
const CHAR_SET_STRING: CharSet = 2; // Construct parsed, can contain strings

impl Pp {
    /**
     * Validate the flags part of a given RegExpLiteral.
     *
     * @param {RegExpValidationState} state The state to validate RegExp.
     * @returns {void}
     */
    fn validate_reg_exp_flags(&self, state: &RegExpValidationState) -> Raise<()> {
        let valid_flags = &state.valid_flags;
        let flags = &state.flags;

        let mut u = false;
        let mut v = false;

        for (i, flag) in flags.char_indices() {
            if !valid_flags.contains(flag) {
                return Err("Invalid regular expression flag".to_owned());
            }
            if flags[i + flag.len_utf8()..].contains(flag) {
                return Err("Duplicate regular expression flag".to_owned());
            }
            if flag == 'u' {
                u = true;
            }
            if flag == 'v' {
                v = true;
            }
        }
        if self.ecma_version >= 15 && u && v {
            return Err("Invalid regular expression flag".to_owned());
        }
        Ok(())
    }

    /**
     * Validate the pattern part of a given RegExpLiteral.
     *
     * @param {RegExpValidationState} state The state to validate RegExp.
     * @returns {void}
     */
    fn validate_reg_exp_pattern(&self, state: &mut RegExpValidationState) -> Raise<()> {
        self.regexp_pattern(state)?;

        // The goal symbol for the parse is |Pattern[~U, ~N]|. If the result of
        // parsing contains a |GroupName|, reparse with the goal symbol
        // |Pattern[~U, +N]| and use this result instead. Throw a *SyntaxError*
        // exception if _P_ did not conform to the grammar, if any elements of _P_
        // were not matched by the parse, or if any Early Error conditions exist.
        if !state.switch_n && self.ecma_version >= 9 && !state.group_names.is_empty() {
            state.switch_n = true;
            self.regexp_pattern(state)?;
        }
        Ok(())
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Pattern
    fn regexp_pattern(&self, state: &mut RegExpValidationState) -> Raise<()> {
        state.pos = 0;
        state.last_int_value = 0.0;
        state.last_string_value = String::new();
        state.last_assertion_is_quantifiable = false;
        state.num_capturing_parens = 0.0;
        state.max_back_reference = 0.0;
        state.group_names = HashMap::new();
        state.back_reference_names.clear();
        state.branch_id = None;
        state.branch_ids.clear();

        self.regexp_disjunction(state)?;

        if state.pos != state.source.len() {
            // Make the same messages as V8.
            if state.eat(0x29 /* ) */) {
                return Err(state.raise("Unmatched ')'"));
            }
            if state.eat(0x5D /* ] */) || state.eat(0x7D /* } */) {
                return Err(state.raise("Lone quantifier brackets"));
            }
        }
        if state.max_back_reference > state.num_capturing_parens {
            return Err(state.raise("Invalid escape"));
        }
        for name in &state.back_reference_names {
            if !state.group_names.contains_key(name) {
                return Err(state.raise("Invalid named capture referenced"));
            }
        }
        Ok(())
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Disjunction
    fn regexp_disjunction(&self, state: &mut RegExpValidationState) -> Raise<()> {
        let track_disjunction = self.ecma_version >= 16;
        if track_disjunction {
            state.branch_id = Some(state.new_branch_id(state.branch_id, None));
        }
        self.regexp_alternative(state)?;
        while state.eat(0x7C /* | */) {
            if track_disjunction {
                state.branch_id = Some(state.sibling(state.branch_id.expect("branchID")));
            }
            self.regexp_alternative(state)?;
        }
        if track_disjunction {
            state.branch_id = state.branch_ids[state.branch_id.expect("branchID")].parent;
        }

        // Make the same message as V8.
        if self.regexp_eat_quantifier(state, true)? {
            return Err(state.raise("Nothing to repeat"));
        }
        if state.eat(0x7B /* { */) {
            return Err(state.raise("Lone quantifier brackets"));
        }
        Ok(())
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Alternative
    fn regexp_alternative(&self, state: &mut RegExpValidationState) -> Raise<()> {
        while state.pos < state.source.len() && self.regexp_eat_term(state)? {}
        Ok(())
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-Term
    fn regexp_eat_term(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        if self.regexp_eat_assertion(state)? {
            // Handle `QuantifiableAssertion Quantifier` alternative.
            // `state.lastAssertionIsQuantifiable` is true if the last eaten Assertion
            // is a QuantifiableAssertion.
            if state.last_assertion_is_quantifiable && self.regexp_eat_quantifier(state, false)? {
                // Make the same message as V8.
                if state.switch_u {
                    return Err(state.raise("Invalid quantifier"));
                }
            }
            return Ok(true);
        }

        if if state.switch_u {
            self.regexp_eat_atom(state)?
        } else {
            self.regexp_eat_extended_atom(state)?
        } {
            self.regexp_eat_quantifier(state, false)?;
            return Ok(true);
        }

        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-Assertion
    fn regexp_eat_assertion(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        state.last_assertion_is_quantifiable = false;

        // ^, $
        if state.eat(0x5E /* ^ */) || state.eat(0x24 /* $ */) {
            return Ok(true);
        }

        // \b \B
        if state.eat(0x5C /* \ */) {
            if state.eat(0x42 /* B */) || state.eat(0x62 /* b */) {
                return Ok(true);
            }
            state.pos = start;
        }

        // Lookahead / Lookbehind
        if state.eat(0x28 /* ( */) && state.eat(0x3F /* ? */) {
            let mut lookbehind = false;
            if self.ecma_version >= 9 {
                lookbehind = state.eat(0x3C /* < */);
            }
            if state.eat(0x3D /* = */) || state.eat(0x21 /* ! */) {
                self.regexp_disjunction(state)?;
                if !state.eat(0x29 /* ) */) {
                    return Err(state.raise("Unterminated group"));
                }
                state.last_assertion_is_quantifiable = !lookbehind;
                return Ok(true);
            }
        }

        state.pos = start;
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Quantifier
    fn regexp_eat_quantifier(
        &self,
        state: &mut RegExpValidationState,
        no_error: bool,
    ) -> Raise<bool> {
        if self.regexp_eat_quantifier_prefix(state, no_error)? {
            state.eat(0x3F /* ? */);
            return Ok(true);
        }
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-QuantifierPrefix
    fn regexp_eat_quantifier_prefix(
        &self,
        state: &mut RegExpValidationState,
        no_error: bool,
    ) -> Raise<bool> {
        Ok(state.eat(0x2A /* * */)
            || state.eat(0x2B /* + */)
            || state.eat(0x3F /* ? */)
            || self.regexp_eat_braced_quantifier(state, no_error)?)
    }
    fn regexp_eat_braced_quantifier(
        &self,
        state: &mut RegExpValidationState,
        no_error: bool,
    ) -> Raise<bool> {
        let start = state.pos;
        if state.eat(0x7B /* { */) {
            // SVELTERS: acorn's `let min = 0` initializer is dead (min is only read after it is set).
            let mut max = -1.0;
            if self.regexp_eat_decimal_digits(state) {
                let min = state.last_int_value;
                if state.eat(0x2C /* , */) && self.regexp_eat_decimal_digits(state) {
                    max = state.last_int_value;
                }
                if state.eat(0x7D /* } */) {
                    // SyntaxError in https://www.ecma-international.org/ecma-262/8.0/#sec-term
                    if max != -1.0 && max < min && !no_error {
                        return Err(state.raise("numbers out of order in {} quantifier"));
                    }
                    return Ok(true);
                }
            }
            if state.switch_u && !no_error {
                return Err(state.raise("Incomplete quantifier"));
            }
            state.pos = start;
        }
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Atom
    fn regexp_eat_atom(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        Ok(self.regexp_eat_pattern_characters(state)
            || state.eat(0x2E /* . */)
            || self.regexp_eat_reverse_solidus_atom_escape(state)?
            || self.regexp_eat_character_class(state)?
            || self.regexp_eat_uncapturing_group(state)?
            || self.regexp_eat_capturing_group(state)?)
    }
    fn regexp_eat_reverse_solidus_atom_escape(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<bool> {
        let start = state.pos;
        if state.eat(0x5C /* \ */) {
            if self.regexp_eat_atom_escape(state)? {
                return Ok(true);
            }
            state.pos = start;
        }
        Ok(false)
    }
    fn regexp_eat_uncapturing_group(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        if state.eat(0x28 /* ( */) {
            if state.eat(0x3F /* ? */) {
                if self.ecma_version >= 16 {
                    let add_modifiers = self.regexp_eat_modifiers(state);
                    let has_hyphen = state.eat(0x2D /* - */);
                    if !add_modifiers.is_empty() || has_hyphen {
                        for (i, modifier) in add_modifiers.char_indices() {
                            if add_modifiers[i + 1..].contains(modifier) {
                                return Err(state.raise("Duplicate regular expression modifiers"));
                            }
                        }
                        if has_hyphen {
                            let remove_modifiers = self.regexp_eat_modifiers(state);
                            if add_modifiers.is_empty()
                                && remove_modifiers.is_empty()
                                && state.current() == 0x3A
                            /* : */
                            {
                                return Err(state.raise("Invalid regular expression modifiers"));
                            }
                            for (i, modifier) in remove_modifiers.char_indices() {
                                if remove_modifiers[i + 1..].contains(modifier)
                                    || add_modifiers.contains(modifier)
                                {
                                    return Err(
                                        state.raise("Duplicate regular expression modifiers")
                                    );
                                }
                            }
                        }
                    }
                }
                if state.eat(0x3A /* : */) {
                    self.regexp_disjunction(state)?;
                    if state.eat(0x29 /* ) */) {
                        return Ok(true);
                    }
                    return Err(state.raise("Unterminated group"));
                }
            }
            state.pos = start;
        }
        Ok(false)
    }
    fn regexp_eat_capturing_group(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        if state.eat(0x28 /* ( */) {
            if self.ecma_version >= 9 {
                self.regexp_group_specifier(state)?;
            } else if state.current() == 0x3F
            /* ? */
            {
                return Err(state.raise("Invalid group"));
            }
            self.regexp_disjunction(state)?;
            if state.eat(0x29 /* ) */) {
                state.num_capturing_parens += 1.0;
                return Ok(true);
            }
            return Err(state.raise("Unterminated group"));
        }
        Ok(false)
    }
    // RegularExpressionModifiers ::
    //   [empty]
    //   RegularExpressionModifiers RegularExpressionModifier
    fn regexp_eat_modifiers(&self, state: &mut RegExpValidationState) -> String {
        let mut modifiers = String::new();
        loop {
            let ch = state.current();
            if !(ch != -1 && is_regular_expression_modifier(ch)) {
                break;
            }
            push_code_point(&mut modifiers, ch as f64);
            state.advance();
        }
        modifiers
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-ExtendedAtom
    fn regexp_eat_extended_atom(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        Ok(state.eat(0x2E /* . */)
            || self.regexp_eat_reverse_solidus_atom_escape(state)?
            || self.regexp_eat_character_class(state)?
            || self.regexp_eat_uncapturing_group(state)?
            || self.regexp_eat_capturing_group(state)?
            || self.regexp_eat_invalid_braced_quantifier(state)?
            || self.regexp_eat_extended_pattern_character(state))
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-InvalidBracedQuantifier
    fn regexp_eat_invalid_braced_quantifier(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<bool> {
        if self.regexp_eat_braced_quantifier(state, true)? {
            return Err(state.raise("Nothing to repeat"));
        }
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-SyntaxCharacter
    fn regexp_eat_syntax_character(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if is_syntax_character(ch) {
            state.last_int_value = ch as f64;
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-PatternCharacter
    // But eat eager.
    fn regexp_eat_pattern_characters(&self, state: &mut RegExpValidationState) -> bool {
        let start = state.pos;
        loop {
            let ch = state.current();
            if !(ch != -1 && !is_syntax_character(ch)) {
                break;
            }
            state.advance();
        }
        state.pos != start
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-ExtendedPatternCharacter
    fn regexp_eat_extended_pattern_character(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if ch != -1
            && ch != 0x24 /* $ */
            && !(0x28 /* ( */..=0x2B /* + */).contains(&ch)
            && ch != 0x2E /* . */
            && ch != 0x3F /* ? */
            && ch != 0x5B /* [ */
            && ch != 0x5E /* ^ */
            && ch != 0x7C
        /* | */
        {
            state.advance();
            return true;
        }
        false
    }

    // GroupSpecifier ::
    //   [empty]
    //   `?` GroupName
    fn regexp_group_specifier(&self, state: &mut RegExpValidationState) -> Raise<()> {
        if state.eat(0x3F /* ? */) {
            if !self.regexp_eat_group_name(state)? {
                return Err(state.raise("Invalid group"));
            }
            let track_disjunction = self.ecma_version >= 16;
            if let Some(known) = state.group_names.get(&state.last_string_value) {
                if track_disjunction {
                    let branch_id = state.branch_id.expect("branchID");
                    for alt_id in known {
                        if !state.separated_from(alt_id.expect("BranchID"), branch_id) {
                            return Err(state.raise("Duplicate capture group name"));
                        }
                    }
                } else {
                    return Err(state.raise("Duplicate capture group name"));
                }
            }
            let entry = state
                .group_names
                .entry(state.last_string_value.clone())
                .or_default();
            if track_disjunction {
                entry.push(state.branch_id);
            }
        }
        Ok(())
    }

    // GroupName ::
    //   `<` RegExpIdentifierName `>`
    // Note: this updates `state.lastStringValue` property with the eaten name.
    fn regexp_eat_group_name(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        state.last_string_value = String::new();
        if state.eat(0x3C /* < */) {
            if self.regexp_eat_reg_exp_identifier_name(state)? && state.eat(0x3E /* > */) {
                return Ok(true);
            }
            return Err(state.raise("Invalid capture group name"));
        }
        Ok(false)
    }

    // RegExpIdentifierName ::
    //   RegExpIdentifierStart
    //   RegExpIdentifierName RegExpIdentifierPart
    // Note: this updates `state.lastStringValue` property with the eaten name.
    fn regexp_eat_reg_exp_identifier_name(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        state.last_string_value = String::new();
        if self.regexp_eat_reg_exp_identifier_start(state)? {
            push_code_point(&mut state.last_string_value, state.last_int_value);
            while self.regexp_eat_reg_exp_identifier_part(state)? {
                push_code_point(&mut state.last_string_value, state.last_int_value);
            }
            return Ok(true);
        }
        Ok(false)
    }

    // RegExpIdentifierStart ::
    //   UnicodeIDStart
    //   `$`
    //   `_`
    //   `\` RegExpUnicodeEscapeSequence[+U]
    fn regexp_eat_reg_exp_identifier_start(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<bool> {
        let start = state.pos;
        let force_u = self.ecma_version >= 11;
        let mut ch = state.current_u(force_u) as f64;
        state.advance_u(force_u);

        if ch == 0x5C as f64 /* \ */ && self.regexp_eat_reg_exp_unicode_escape_sequence(state, force_u)?
        {
            ch = state.last_int_value;
        }
        if is_reg_exp_identifier_start(ch) {
            state.last_int_value = ch;
            return Ok(true);
        }

        state.pos = start;
        Ok(false)
    }

    // RegExpIdentifierPart ::
    //   UnicodeIDContinue
    //   `$`
    //   `_`
    //   `\` RegExpUnicodeEscapeSequence[+U]
    //   <ZWNJ>
    //   <ZWJ>
    fn regexp_eat_reg_exp_identifier_part(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        let force_u = self.ecma_version >= 11;
        let mut ch = state.current_u(force_u) as f64;
        state.advance_u(force_u);

        if ch == 0x5C as f64 /* \ */ && self.regexp_eat_reg_exp_unicode_escape_sequence(state, force_u)?
        {
            ch = state.last_int_value;
        }
        if is_reg_exp_identifier_part(ch) {
            state.last_int_value = ch;
            return Ok(true);
        }

        state.pos = start;
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-AtomEscape
    fn regexp_eat_atom_escape(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        if self.regexp_eat_back_reference(state)
            || self.regexp_eat_character_class_escape(state)? != CHAR_SET_NONE
            || self.regexp_eat_character_escape(state)?
            || (state.switch_n && self.regexp_eat_k_group_name(state)?)
        {
            return Ok(true);
        }
        if state.switch_u {
            // Make the same message as V8.
            if state.current() == 0x63
            /* c */
            {
                return Err(state.raise("Invalid unicode escape"));
            }
            return Err(state.raise("Invalid escape"));
        }
        Ok(false)
    }
    fn regexp_eat_back_reference(&self, state: &mut RegExpValidationState) -> bool {
        let start = state.pos;
        if self.regexp_eat_decimal_escape(state) {
            let n = state.last_int_value;
            if state.switch_u {
                // For SyntaxError in https://www.ecma-international.org/ecma-262/8.0/#sec-atomescape
                if n > state.max_back_reference {
                    state.max_back_reference = n;
                }
                return true;
            }
            if n <= state.num_capturing_parens {
                return true;
            }
            state.pos = start;
        }
        false
    }
    fn regexp_eat_k_group_name(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        if state.eat(0x6B /* k */) {
            if self.regexp_eat_group_name(state)? {
                state
                    .back_reference_names
                    .push(state.last_string_value.clone());
                return Ok(true);
            }
            return Err(state.raise("Invalid named reference"));
        }
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-CharacterEscape
    fn regexp_eat_character_escape(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        Ok(self.regexp_eat_control_escape(state)
            || self.regexp_eat_c_control_letter(state)
            || self.regexp_eat_zero(state)
            || self.regexp_eat_hex_escape_sequence(state)?
            || self.regexp_eat_reg_exp_unicode_escape_sequence(state, false)?
            || (!state.switch_u && self.regexp_eat_legacy_octal_escape_sequence(state))
            || self.regexp_eat_identity_escape(state))
    }
    fn regexp_eat_c_control_letter(&self, state: &mut RegExpValidationState) -> bool {
        let start = state.pos;
        if state.eat(0x63 /* c */) {
            if self.regexp_eat_control_letter(state) {
                return true;
            }
            state.pos = start;
        }
        false
    }
    fn regexp_eat_zero(&self, state: &mut RegExpValidationState) -> bool {
        if state.current() == 0x30 /* 0 */ && !is_decimal_digit(state.lookahead()) {
            state.last_int_value = 0.0;
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-ControlEscape
    fn regexp_eat_control_escape(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if ch == 0x74
        /* t */
        {
            state.last_int_value = 0x09 as f64; /* \t */
            state.advance();
            return true;
        }
        if ch == 0x6E
        /* n */
        {
            state.last_int_value = 0x0A as f64; /* \n */
            state.advance();
            return true;
        }
        if ch == 0x76
        /* v */
        {
            state.last_int_value = 0x0B as f64; /* \v */
            state.advance();
            return true;
        }
        if ch == 0x66
        /* f */
        {
            state.last_int_value = 0x0C as f64; /* \f */
            state.advance();
            return true;
        }
        if ch == 0x72
        /* r */
        {
            state.last_int_value = 0x0D as f64; /* \r */
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-ControlLetter
    fn regexp_eat_control_letter(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if is_control_letter(ch) {
            state.last_int_value = (ch % 0x20) as f64;
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-RegExpUnicodeEscapeSequence
    fn regexp_eat_reg_exp_unicode_escape_sequence(
        &self,
        state: &mut RegExpValidationState,
        force_u: bool,
    ) -> Raise<bool> {
        let start = state.pos;
        let switch_u = force_u || state.switch_u;

        if state.eat(0x75 /* u */) {
            if self.regexp_eat_fixed_hex_digits(state, 4) {
                let lead = state.last_int_value;
                if switch_u && (0xD800 as f64..=0xDBFF as f64).contains(&lead) {
                    let lead_surrogate_end = state.pos;
                    if state.eat(0x5C /* \ */)
                        && state.eat(0x75 /* u */)
                        && self.regexp_eat_fixed_hex_digits(state, 4)
                    {
                        let trail = state.last_int_value;
                        if (0xDC00 as f64..=0xDFFF as f64).contains(&trail) {
                            state.last_int_value = (lead - 0xD800 as f64) * 0x400 as f64
                                + (trail - 0xDC00 as f64)
                                + 0x10000 as f64;
                            return Ok(true);
                        }
                    }
                    state.pos = lead_surrogate_end;
                    state.last_int_value = lead;
                }
                return Ok(true);
            }
            if switch_u
                && state.eat(0x7B /* { */)
                && self.regexp_eat_hex_digits(state)
                && state.eat(0x7D /* } */)
                && is_valid_unicode(state.last_int_value)
            {
                return Ok(true);
            }
            if switch_u {
                return Err(state.raise("Invalid unicode escape"));
            }
            state.pos = start;
        }

        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-IdentityEscape
    fn regexp_eat_identity_escape(&self, state: &mut RegExpValidationState) -> bool {
        if state.switch_u {
            if self.regexp_eat_syntax_character(state) {
                return true;
            }
            if state.eat(0x2F /* / */) {
                state.last_int_value = 0x2F as f64; /* / */
                return true;
            }
            return false;
        }

        let ch = state.current();
        if ch != 0x63 /* c */ && (!state.switch_n || ch != 0x6B/* k */) {
            state.last_int_value = ch as f64;
            state.advance();
            return true;
        }

        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-DecimalEscape
    fn regexp_eat_decimal_escape(&self, state: &mut RegExpValidationState) -> bool {
        state.last_int_value = 0.0;
        let mut ch = state.current();
        if (0x31 /* 1 */..=0x39/* 9 */).contains(&ch) {
            loop {
                state.last_int_value = 10.0 * state.last_int_value + (ch - 0x30/* 0 */) as f64;
                state.advance();
                ch = state.current();
                if !(0x30 /* 0 */..=0x39/* 9 */).contains(&ch) {
                    break;
                }
            }
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-CharacterClassEscape
    fn regexp_eat_character_class_escape(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<CharSet> {
        let ch = state.current();

        if is_character_class_escape(ch) {
            state.last_int_value = -1.0;
            state.advance();
            return Ok(CHAR_SET_OK);
        }

        // SVELTERS: acorn assigns `negate` inside the condition; it is only read when that holds.
        let negate = ch == 0x50 /* P */;
        if state.switch_u && self.ecma_version >= 9 && (negate || ch == 0x70/* p */) {
            state.last_int_value = -1.0;
            state.advance();
            if state.eat(0x7B /* { */) {
                let result = self.regexp_eat_unicode_property_value_expression(state)?;
                if result != CHAR_SET_NONE && state.eat(0x7D /* } */) {
                    if negate && result == CHAR_SET_STRING {
                        return Err(state.raise("Invalid property name"));
                    }
                    return Ok(result);
                }
            }
            return Err(state.raise("Invalid property name"));
        }

        Ok(CHAR_SET_NONE)
    }

    // UnicodePropertyValueExpression ::
    //   UnicodePropertyName `=` UnicodePropertyValue
    //   LoneUnicodePropertyNameOrValue
    fn regexp_eat_unicode_property_value_expression(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<CharSet> {
        let start = state.pos;

        // UnicodePropertyName `=` UnicodePropertyValue
        if self.regexp_eat_unicode_property_name(state) && state.eat(0x3D /* = */) {
            let name = state.last_string_value.clone();
            if self.regexp_eat_unicode_property_value(state) {
                let value = state.last_string_value.clone();
                self.regexp_validate_unicode_property_name_and_value(state, &name, &value)?;
                return Ok(CHAR_SET_OK);
            }
        }
        state.pos = start;

        // LoneUnicodePropertyNameOrValue
        if self.regexp_eat_lone_unicode_property_name_or_value(state) {
            let name_or_value = state.last_string_value.clone();
            return self.regexp_validate_unicode_property_name_or_value(state, &name_or_value);
        }
        Ok(CHAR_SET_NONE)
    }

    fn regexp_validate_unicode_property_name_and_value(
        &self,
        state: &RegExpValidationState,
        name: &str,
        value: &str,
    ) -> Raise<()> {
        let unicode_properties = state.unicode_properties.expect("unicodeProperties");
        let Some(non_binary) = unicode_properties.non_binary.get(name) else {
            return Err(state.raise("Invalid property name"));
        };
        if !non_binary.test(value) {
            return Err(state.raise("Invalid property value"));
        }
        Ok(())
    }

    fn regexp_validate_unicode_property_name_or_value(
        &self,
        state: &RegExpValidationState,
        name_or_value: &str,
    ) -> Raise<CharSet> {
        let unicode_properties = state.unicode_properties.expect("unicodeProperties");
        if unicode_properties.binary.test(name_or_value) {
            return Ok(CHAR_SET_OK);
        }
        if state.switch_v && unicode_properties.binary_of_strings.test(name_or_value) {
            return Ok(CHAR_SET_STRING);
        }
        Err(state.raise("Invalid property name"))
    }

    // UnicodePropertyName ::
    //   UnicodePropertyNameCharacters
    fn regexp_eat_unicode_property_name(&self, state: &mut RegExpValidationState) -> bool {
        state.last_string_value = String::new();
        loop {
            let ch = state.current();
            if !is_unicode_property_name_character(ch) {
                break;
            }
            push_code_point(&mut state.last_string_value, ch as f64);
            state.advance();
        }
        !state.last_string_value.is_empty()
    }

    // UnicodePropertyValue ::
    //   UnicodePropertyValueCharacters
    fn regexp_eat_unicode_property_value(&self, state: &mut RegExpValidationState) -> bool {
        state.last_string_value = String::new();
        loop {
            let ch = state.current();
            if !is_unicode_property_value_character(ch) {
                break;
            }
            push_code_point(&mut state.last_string_value, ch as f64);
            state.advance();
        }
        !state.last_string_value.is_empty()
    }

    // LoneUnicodePropertyNameOrValue ::
    //   UnicodePropertyValueCharacters
    fn regexp_eat_lone_unicode_property_name_or_value(
        &self,
        state: &mut RegExpValidationState,
    ) -> bool {
        self.regexp_eat_unicode_property_value(state)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-CharacterClass
    fn regexp_eat_character_class(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        if state.eat(0x5B /* [ */) {
            let negate = state.eat(0x5E /* ^ */);
            let result = self.regexp_class_contents(state)?;
            if !state.eat(0x5D /* ] */) {
                return Err(state.raise("Unterminated character class"));
            }
            if negate && result == CHAR_SET_STRING {
                return Err(state.raise("Negated character class may contain strings"));
            }
            return Ok(true);
        }
        Ok(false)
    }

    // https://tc39.es/ecma262/#prod-ClassContents
    // https://www.ecma-international.org/ecma-262/8.0/#prod-ClassRanges
    fn regexp_class_contents(&self, state: &mut RegExpValidationState) -> Raise<CharSet> {
        if state.current() == 0x5D
        /* ] */
        {
            return Ok(CHAR_SET_OK);
        }
        if state.switch_v {
            return self.regexp_class_set_expression(state);
        }
        self.regexp_non_empty_class_ranges(state)?;
        Ok(CHAR_SET_OK)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-NonemptyClassRanges
    // https://www.ecma-international.org/ecma-262/8.0/#prod-NonemptyClassRangesNoDash
    fn regexp_non_empty_class_ranges(&self, state: &mut RegExpValidationState) -> Raise<()> {
        while self.regexp_eat_class_atom(state)? {
            let left = state.last_int_value;
            if state.eat(0x2D /* - */) && self.regexp_eat_class_atom(state)? {
                let right = state.last_int_value;
                if state.switch_u && (left == -1.0 || right == -1.0) {
                    return Err(state.raise("Invalid character class"));
                }
                if left != -1.0 && right != -1.0 && left > right {
                    return Err(state.raise("Range out of order in character class"));
                }
            }
        }
        Ok(())
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-ClassAtom
    // https://www.ecma-international.org/ecma-262/8.0/#prod-ClassAtomNoDash
    fn regexp_eat_class_atom(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;

        if state.eat(0x5C /* \ */) {
            if self.regexp_eat_class_escape(state)? {
                return Ok(true);
            }
            if state.switch_u {
                // Make the same message as V8.
                let ch = state.current();
                if ch == 0x63 /* c */ || is_octal_digit(ch) {
                    return Err(state.raise("Invalid class escape"));
                }
                return Err(state.raise("Invalid escape"));
            }
            state.pos = start;
        }

        let ch = state.current();
        if ch != 0x5D
        /* ] */
        {
            state.last_int_value = ch as f64;
            state.advance();
            return Ok(true);
        }

        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-ClassEscape
    fn regexp_eat_class_escape(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;

        if state.eat(0x62 /* b */) {
            state.last_int_value = 0x08 as f64; /* <BS> */
            return Ok(true);
        }

        if state.switch_u && state.eat(0x2D /* - */) {
            state.last_int_value = 0x2D as f64; /* - */
            return Ok(true);
        }

        if !state.switch_u && state.eat(0x63 /* c */) {
            if self.regexp_eat_class_control_letter(state) {
                return Ok(true);
            }
            state.pos = start;
        }

        Ok(
            self.regexp_eat_character_class_escape(state)? != CHAR_SET_NONE
                || self.regexp_eat_character_escape(state)?,
        )
    }

    // https://tc39.es/ecma262/#prod-ClassSetExpression
    // https://tc39.es/ecma262/#prod-ClassUnion
    // https://tc39.es/ecma262/#prod-ClassIntersection
    // https://tc39.es/ecma262/#prod-ClassSubtraction
    fn regexp_class_set_expression(&self, state: &mut RegExpValidationState) -> Raise<CharSet> {
        let mut result = CHAR_SET_OK;
        let mut sub_result;
        if self.regexp_eat_class_set_range(state)? {
            // Continue with ClassUnion processing.
        } else {
            sub_result = self.regexp_eat_class_set_operand(state)?;
            if sub_result == CHAR_SET_NONE {
                return Err(state.raise("Invalid character in character class"));
            }
            if sub_result == CHAR_SET_STRING {
                result = CHAR_SET_STRING;
            }
            // https://tc39.es/ecma262/#prod-ClassIntersection
            let start = state.pos;
            while state.eat_chars(&[0x26, 0x26] /* && */) {
                if state.current() != 0x26 /* & */ && {
                    sub_result = self.regexp_eat_class_set_operand(state)?;
                    sub_result != CHAR_SET_NONE
                } {
                    if sub_result != CHAR_SET_STRING {
                        result = CHAR_SET_OK;
                    }
                    continue;
                }
                return Err(state.raise("Invalid character in character class"));
            }
            if start != state.pos {
                return Ok(result);
            }
            // https://tc39.es/ecma262/#prod-ClassSubtraction
            while state.eat_chars(&[0x2D, 0x2D] /* -- */) {
                if self.regexp_eat_class_set_operand(state)? != CHAR_SET_NONE {
                    continue;
                }
                return Err(state.raise("Invalid character in character class"));
            }
            if start != state.pos {
                return Ok(result);
            }
        }
        // https://tc39.es/ecma262/#prod-ClassUnion
        loop {
            if self.regexp_eat_class_set_range(state)? {
                continue;
            }
            sub_result = self.regexp_eat_class_set_operand(state)?;
            if sub_result == CHAR_SET_NONE {
                return Ok(result);
            }
            if sub_result == CHAR_SET_STRING {
                result = CHAR_SET_STRING;
            }
        }
    }

    // https://tc39.es/ecma262/#prod-ClassSetRange
    fn regexp_eat_class_set_range(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        if self.regexp_eat_class_set_character(state)? {
            let left = state.last_int_value;
            if state.eat(0x2D /* - */) && self.regexp_eat_class_set_character(state)? {
                let right = state.last_int_value;
                if left != -1.0 && right != -1.0 && left > right {
                    return Err(state.raise("Range out of order in character class"));
                }
                return Ok(true);
            }
            state.pos = start;
        }
        Ok(false)
    }

    // https://tc39.es/ecma262/#prod-ClassSetOperand
    fn regexp_eat_class_set_operand(&self, state: &mut RegExpValidationState) -> Raise<CharSet> {
        if self.regexp_eat_class_set_character(state)? {
            return Ok(CHAR_SET_OK);
        }
        let result = self.regexp_eat_class_string_disjunction(state)?;
        if result != CHAR_SET_NONE {
            return Ok(result);
        }
        self.regexp_eat_nested_class(state)
    }

    // https://tc39.es/ecma262/#prod-NestedClass
    fn regexp_eat_nested_class(&self, state: &mut RegExpValidationState) -> Raise<CharSet> {
        let start = state.pos;
        if state.eat(0x5B /* [ */) {
            let negate = state.eat(0x5E /* ^ */);
            let result = self.regexp_class_contents(state)?;
            if state.eat(0x5D /* ] */) {
                if negate && result == CHAR_SET_STRING {
                    return Err(state.raise("Negated character class may contain strings"));
                }
                return Ok(result);
            }
            state.pos = start;
        }
        if state.eat(0x5C /* \ */) {
            let result = self.regexp_eat_character_class_escape(state)?;
            if result != CHAR_SET_NONE {
                return Ok(result);
            }
            state.pos = start;
        }
        Ok(CHAR_SET_NONE) // null
    }

    // https://tc39.es/ecma262/#prod-ClassStringDisjunction
    fn regexp_eat_class_string_disjunction(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<CharSet> {
        let start = state.pos;
        if state.eat_chars(&[0x5C, 0x71] /* \q */) {
            if state.eat(0x7B /* { */) {
                let result = self.regexp_class_string_disjunction_contents(state)?;
                if state.eat(0x7D /* } */) {
                    return Ok(result);
                }
            } else {
                // Make the same message as V8.
                return Err(state.raise("Invalid escape"));
            }
            state.pos = start;
        }
        Ok(CHAR_SET_NONE) // null
    }

    // https://tc39.es/ecma262/#prod-ClassStringDisjunctionContents
    fn regexp_class_string_disjunction_contents(
        &self,
        state: &mut RegExpValidationState,
    ) -> Raise<CharSet> {
        let mut result = self.regexp_class_string(state)?;
        while state.eat(0x7C /* | */) {
            if self.regexp_class_string(state)? == CHAR_SET_STRING {
                result = CHAR_SET_STRING;
            }
        }
        Ok(result)
    }

    // https://tc39.es/ecma262/#prod-ClassString
    // https://tc39.es/ecma262/#prod-NonEmptyClassString
    fn regexp_class_string(&self, state: &mut RegExpValidationState) -> Raise<CharSet> {
        let mut count = 0;
        while self.regexp_eat_class_set_character(state)? {
            count += 1;
        }
        Ok(if count == 1 {
            CHAR_SET_OK
        } else {
            CHAR_SET_STRING
        })
    }

    // https://tc39.es/ecma262/#prod-ClassSetCharacter
    fn regexp_eat_class_set_character(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        if state.eat(0x5C /* \ */) {
            if self.regexp_eat_character_escape(state)?
                || self.regexp_eat_class_set_reserved_punctuator(state)
            {
                return Ok(true);
            }
            if state.eat(0x62 /* b */) {
                state.last_int_value = 0x08 as f64; /* <BS> */
                return Ok(true);
            }
            state.pos = start;
            return Ok(false);
        }
        let ch = state.current();
        if ch < 0
            || ch == state.lookahead() && is_class_set_reserved_double_punctuator_character(ch)
        {
            return Ok(false);
        }
        if is_class_set_syntax_character(ch) {
            return Ok(false);
        }
        state.advance();
        state.last_int_value = ch as f64;
        Ok(true)
    }

    // https://tc39.es/ecma262/#prod-ClassSetReservedPunctuator
    fn regexp_eat_class_set_reserved_punctuator(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if is_class_set_reserved_punctuator(ch) {
            state.last_int_value = ch as f64;
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-ClassControlLetter
    fn regexp_eat_class_control_letter(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if is_decimal_digit(ch) || ch == 0x5F
        /* _ */
        {
            state.last_int_value = (ch % 0x20) as f64;
            state.advance();
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-HexEscapeSequence
    fn regexp_eat_hex_escape_sequence(&self, state: &mut RegExpValidationState) -> Raise<bool> {
        let start = state.pos;
        if state.eat(0x78 /* x */) {
            if self.regexp_eat_fixed_hex_digits(state, 2) {
                return Ok(true);
            }
            if state.switch_u {
                return Err(state.raise("Invalid escape"));
            }
            state.pos = start;
        }
        Ok(false)
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-DecimalDigits
    fn regexp_eat_decimal_digits(&self, state: &mut RegExpValidationState) -> bool {
        let start = state.pos;
        state.last_int_value = 0.0;
        loop {
            let ch = state.current();
            if !is_decimal_digit(ch) {
                break;
            }
            state.last_int_value = 10.0 * state.last_int_value + (ch - 0x30/* 0 */) as f64;
            state.advance();
        }
        state.pos != start
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-HexDigits
    fn regexp_eat_hex_digits(&self, state: &mut RegExpValidationState) -> bool {
        let start = state.pos;
        state.last_int_value = 0.0;
        loop {
            let ch = state.current();
            if !is_hex_digit(ch) {
                break;
            }
            state.last_int_value = 16.0 * state.last_int_value + hex_to_int(ch) as f64;
            state.advance();
        }
        state.pos != start
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-annexB-LegacyOctalEscapeSequence
    // Allows only 0-377(octal) i.e. 0-255(decimal).
    fn regexp_eat_legacy_octal_escape_sequence(&self, state: &mut RegExpValidationState) -> bool {
        if self.regexp_eat_octal_digit(state) {
            let n1 = state.last_int_value;
            if self.regexp_eat_octal_digit(state) {
                let n2 = state.last_int_value;
                if n1 <= 3.0 && self.regexp_eat_octal_digit(state) {
                    let n3 = state.last_int_value;
                    state.last_int_value = n1 * 64.0 + n2 * 8.0 + n3;
                } else {
                    state.last_int_value = n1 * 8.0 + n2;
                }
            } else {
                state.last_int_value = n1;
            }
            return true;
        }
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-OctalDigit
    fn regexp_eat_octal_digit(&self, state: &mut RegExpValidationState) -> bool {
        let ch = state.current();
        if is_octal_digit(ch) {
            state.last_int_value = (ch - 0x30/* 0 */) as f64;
            state.advance();
            return true;
        }
        state.last_int_value = 0.0;
        false
    }

    // https://www.ecma-international.org/ecma-262/8.0/#prod-Hex4Digits
    // https://www.ecma-international.org/ecma-262/8.0/#prod-HexDigit
    // And HexDigit HexDigit in https://www.ecma-international.org/ecma-262/8.0/#prod-HexEscapeSequence
    fn regexp_eat_fixed_hex_digits(
        &self,
        state: &mut RegExpValidationState,
        length: usize,
    ) -> bool {
        let start = state.pos;
        state.last_int_value = 0.0;
        for _ in 0..length {
            let ch = state.current();
            if !is_hex_digit(ch) {
                state.pos = start;
                return false;
            }
            state.last_int_value = 16.0 * state.last_int_value + hex_to_int(ch) as f64;
            state.advance();
        }
        true
    }
}

// RegularExpressionModifier :: one of
//   `i` `m` `s`
fn is_regular_expression_modifier(ch: i32) -> bool {
    ch == 0x69 /* i */ || ch == 0x6d /* m */ || ch == 0x73 /* s */
}

fn is_syntax_character(ch: i32) -> bool {
    ch == 0x24 /* $ */
        || (0x28 /* ( */..=0x2B /* + */).contains(&ch)
        || ch == 0x2E /* . */
        || ch == 0x3F /* ? */
        || (0x5B /* [ */..=0x5E /* ^ */).contains(&ch)
        || (0x7B /* { */..=0x7D /* } */).contains(&ch)
}

/// acorn `isIdentifierStart(code, true)` / `isIdentifierChar(code, true)` take any number;
/// non-scalar values (lone surrogates, -1, out of range) are never identifier characters there.
fn identifier_code(ch: f64) -> Option<char> {
    if ch < 0.0 {
        None
    } else {
        char::from_u32(ch as u32)
    }
}

fn is_reg_exp_identifier_start(ch: f64) -> bool {
    identifier_code(ch).is_some_and(super::is_identifier_start) || ch == 0x24 as f64 /* $ */ || ch == 0x5F as f64 /* _ */
}

fn is_reg_exp_identifier_part(ch: f64) -> bool {
    identifier_code(ch).is_some_and(super::is_identifier_char)
        || ch == 0x24 as f64 /* $ */
        || ch == 0x5F as f64 /* _ */
        || ch == 0x200C as f64 /* <ZWNJ> */
        || ch == 0x200D as f64 /* <ZWJ> */
}

fn is_control_letter(ch: i32) -> bool {
    (0x41 /* A */..=0x5A/* Z */).contains(&ch) || (0x61 /* a */..=0x7A/* z */).contains(&ch)
}

fn is_valid_unicode(ch: f64) -> bool {
    (0.0..=0x10FFFF as f64).contains(&ch)
}

fn is_character_class_escape(ch: i32) -> bool {
    ch == 0x64 /* d */
        || ch == 0x44 /* D */
        || ch == 0x73 /* s */
        || ch == 0x53 /* S */
        || ch == 0x77 /* w */
        || ch == 0x57 /* W */
}

fn is_unicode_property_name_character(ch: i32) -> bool {
    is_control_letter(ch) || ch == 0x5F /* _ */
}

fn is_unicode_property_value_character(ch: i32) -> bool {
    is_unicode_property_name_character(ch) || is_decimal_digit(ch)
}

// https://tc39.es/ecma262/#prod-ClassSetReservedDoublePunctuator
fn is_class_set_reserved_double_punctuator_character(ch: i32) -> bool {
    ch == 0x21 /* ! */
        || (0x23 /* # */..=0x26 /* & */).contains(&ch)
        || (0x2A /* * */..=0x2C /* , */).contains(&ch)
        || ch == 0x2E /* . */
        || (0x3A /* : */..=0x40 /* @ */).contains(&ch)
        || ch == 0x5E /* ^ */
        || ch == 0x60 /* ` */
        || ch == 0x7E /* ~ */
}

// https://tc39.es/ecma262/#prod-ClassSetSyntaxCharacter
fn is_class_set_syntax_character(ch: i32) -> bool {
    ch == 0x28 /* ( */
        || ch == 0x29 /* ) */
        || ch == 0x2D /* - */
        || ch == 0x2F /* / */
        || (0x5B /* [ */..=0x5D /* ] */).contains(&ch)
        || (0x7B /* { */..=0x7D /* } */).contains(&ch)
}

// https://tc39.es/ecma262/#prod-ClassSetReservedPunctuator
fn is_class_set_reserved_punctuator(ch: i32) -> bool {
    ch == 0x21 /* ! */
        || ch == 0x23 /* # */
        || ch == 0x25 /* % */
        || ch == 0x26 /* & */
        || ch == 0x2C /* , */
        || ch == 0x2D /* - */
        || (0x3A /* : */..=0x3E /* > */).contains(&ch)
        || ch == 0x40 /* @ */
        || ch == 0x60 /* ` */
        || ch == 0x7E /* ~ */
}

fn is_decimal_digit(ch: i32) -> bool {
    (0x30 /* 0 */..=0x39/* 9 */).contains(&ch)
}

fn is_hex_digit(ch: i32) -> bool {
    (0x30 /* 0 */..=0x39/* 9 */).contains(&ch)
        || (0x41 /* A */..=0x46/* F */).contains(&ch)
        || (0x61 /* a */..=0x66/* f */).contains(&ch)
}

fn hex_to_int(ch: i32) -> i32 {
    if (0x41 /* A */..=0x46/* F */).contains(&ch) {
        return 10 + (ch - 0x41/* A */);
    }
    if (0x61 /* a */..=0x66/* f */).contains(&ch) {
        return 10 + (ch - 0x61/* a */);
    }
    ch - 0x30 /* 0 */
}

fn is_octal_digit(ch: i32) -> bool {
    (0x30 /* 0 */..=0x37/* 7 */).contains(&ch)
}
