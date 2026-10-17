//! Virtualized lists: `<ListView>` imported from `@svelters/lists`.
//!
//! A virtualized list is an explicit choice in the source: `{#each}` builds every item as children
//! of a container; `<ListView items key row>` (the JS pipeline's component, resolved by the native
//! build to `@svelters/native-runtime/lists`) drives a native `virtuallist` whose toolkit asks for
//! row content only for the rows it shows (docs/notes/native-compiler/runtime-contract.md
//! "Virtualized lists"). The component is ordinary JS at runtime; this module checks what the
//! compiler can know statically about a use of it:
//!
//! - a `row` snippet (`{#snippet row(item, index)}` child, or a `row={...}` prop) is required
//!   (`list_row_missing`) and takes at most `(item, index)` (`list_row_params`);
//! - other children are not rendered by the list (`list_children`);
//! - each root element of an inline `row` snippet must be a non-toplevel kind (`list_row_toplevel`)
//!   placeable in the row outlet, a native `box` (the spec's `place_child` codes, e.g.
//!   `slot_rejects_child`);
//! - `bind:` only `selected` and `this` (`list_bind_invalid`); static `layout` is `list` or
//!   `grid`, static `selection` is `none` or `single` (`list_prop_invalid`).

use super::{Lower, TagParts, root_elements, sp, static_text};
use crate::estree::{ImportSpecifierKind, LiteralValue, ModuleExportName, Spanned, Statement};
use crate::svelte::ast::{AttributeLike, FragmentNode, Root};
use std::collections::HashSet;

/// The module whose `ListView` export is the virtualized list.
pub const LISTS_MODULE: &str = "@svelters/lists";
/// The native kind that hosts one row's content (the outlet created per row slot).
pub const ROW_OUTLET_KIND: &str = "box";

/// Local names bound to `ListView` from [`LISTS_MODULE`] in the instance or module script.
pub fn list_view_names(root: &Root) -> HashSet<String> {
    let mut out = HashSet::new();
    for script in [&root.instance, &root.module].into_iter().flatten() {
        for st in &script.content.body {
            let Statement::ImportDeclaration(d) = st else { continue };
            let crate::estree::LiteralNode::Literal(src) = &d.source;
            if !matches!(&src.value, LiteralValue::String(s) if s == LISTS_MODULE) {
                continue;
            }
            for s in &d.specifiers {
                if let ImportSpecifierKind::ImportSpecifier(s) = s {
                    let imported = match &s.imported {
                        ModuleExportName::Identifier(i) => i.name.as_str(),
                        ModuleExportName::Literal(l) => match &l.value {
                            LiteralValue::String(v) => v.as_str(),
                            _ => continue,
                        },
                    };
                    if imported == "ListView" {
                        out.insert(s.local.name.clone());
                    }
                }
            }
        }
    }
    out
}

impl<'a> Lower<'a, '_, '_> {
    /// Static checks of a `<ListView>` use (see the module docs).
    pub(super) fn check_list_view(&mut self, el: &TagParts<'a>) {
        let tag = sp(el.name_span);
        let mut has_row = false;
        for a in el.attributes {
            match a {
                AttributeLike::Attribute(at) => {
                    has_row |= at.name == "row";
                    let allowed: &[&str] = match at.name.as_str() {
                        "layout" => &["list", "grid"],
                        "selection" => &["none", "single"],
                        _ => continue,
                    };
                    if let Some(v) = static_text(at)
                        && !allowed.contains(&v.as_str())
                    {
                        self.diags.error(
                            "list_prop_invalid",
                            sp(at.span()),
                            format!("<{}> `{}` must be one of {}, got `{v}`", el.name, at.name, allowed.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ")),
                        );
                    }
                }
                AttributeLike::SpreadAttribute(_) => has_row = true, // may carry `row`
                AttributeLike::BindDirective(d) if d.name != "selected" && d.name != "this" => {
                    self.diags.error(
                        "list_bind_invalid",
                        sp(d.span()),
                        format!("<{}> has no bindable `{}`: only `bind:selected` (the selected position) and `bind:this` (for `scrollTo`)", el.name, d.name),
                    );
                }
                _ => {}
            }
        }
        let outlet = self.sem.resolve_element(ROW_OUTLET_KIND).ok();
        let mut children = false;
        for n in &el.fragment.nodes {
            match n {
                FragmentNode::SnippetBlock(b) if b.expression.name == "row" => {
                    has_row = true;
                    if b.parameters.len() > 2 {
                        self.diags.error("list_row_params", (b.expression.start, b.expression.end), "the `row` snippet of a virtualized list takes at most `(item, index)`");
                    }
                    let Some(outlet) = &outlet else { continue };
                    let mut roots = Vec::new();
                    root_elements(&b.body, &mut roots);
                    for r in roots {
                        if self.list_views.contains(r.name.as_str()) {
                            continue;
                        }
                        let Ok(er) = self.sem.resolve_element(&r.name) else { continue }; // reported at the element
                        if self.sem.lifetime(&er).lifetime == crate::semantics::Lifetime::Toplevel {
                            self.diags.native_error(
                                "list_row_toplevel",
                                sp(r.name_span),
                                format!("<{}> is a toplevel on {}; a row of a virtualized list is content inside the list (a <{ROW_OUTLET_KIND}> per row)", r.name, self.sem.backend()),
                            );
                            continue;
                        }
                        if let Err(u) = self.sem.place_child(outlet, &er, None) {
                            let u = crate::semantics::Unresolved {
                                message: format!("<{}> is a root element of the `row` snippet, which a virtualized list renders into a <{ROW_OUTLET_KIND}> per row: {}", r.name, u.message),
                                ..u
                            };
                            u.report(self.diags, sp(r.name_span));
                        }
                    }
                }
                FragmentNode::SnippetBlock(_) | FragmentNode::Comment(_) | FragmentNode::ConstTag(_) => {}
                FragmentNode::Text(t) if t.data.trim().is_empty() => {}
                _ => children = true,
            }
        }
        if children {
            self.diags.error(
                "list_children",
                (el.span.start, el.span.end),
                format!("<{}> renders only its `row` snippet; put the row content in `{{#snippet row(item, index)}}` (use {{#each}} for ordinary children of a container)", el.name),
            );
        }
        if !has_row {
            self.diags.error("list_row_missing", tag, format!("<{}> needs a `row` snippet: `{{#snippet row(item, index)}}...{{/snippet}}` inside the tag", el.name));
        }
    }
}
