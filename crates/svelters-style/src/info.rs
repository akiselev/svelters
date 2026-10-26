//! DZ0: what a designer's or inspector's "matched rules" pane shows for a node: plain data, shared by
//! every backend's style engine (svelters-egui's scene, skelter-core's document) and
//! svelters-automation's `style.matched`.

/// DZ0: a place in a component's `<style>`: the component's `.svelte` path (as the compiler got
/// it) and a byte offset from the start of the `<style>` element's content. Relative to the style
/// content so it stays right while the template above it is edited: add the content start of the
/// file as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleSource {
    pub file: String,
    pub style_offset: u32,
}

/// DZ0: one declaration of a [`MatchedRuleInfo`].
#[derive(Clone, Debug, PartialEq)]
pub struct DeclInfo {
    pub name: String,
    pub value: String,
    pub important: bool,
    /// Where it is in the component's `<style>` (dev builds; `None` for theme and host sheets).
    pub source: Option<StyleSource>,
    /// DZ0b2: it wins the cascade for its property (for a shorthand, for at least one longhand)
    /// over every other rule and the node's `style=` and forced declarations. A prop the runtime
    /// set still wins over it (CSS.md §2.8).
    pub winner: bool,
}

/// DZ0: a rule that matches a node, as a designer's or inspector's "matched rules" pane shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchedRuleInfo {
    /// The sheet's name (a component's logical id, `@theme…`, a host sheet); `None` for a
    /// generated utility rule.
    pub sheet: Option<String>,
    /// The selectors as CSS (after nesting; the scoping class included).
    pub selectors: String,
    /// The highest specificity among the selectors that matched.
    pub specificity: u32,
    /// The rule's position in the sheet text (1-based line, UTF-16 column).
    pub line: u32,
    pub column: u32,
    /// Where the rule is in the component's `<style>` (dev builds).
    pub source: Option<StyleSource>,
    pub decls: Vec<DeclInfo>,
}
