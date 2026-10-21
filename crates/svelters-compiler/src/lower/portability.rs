//! Portability mode (`svelters-compile --backend gtk,qt,appkit,winui`, REACTIVITY.md sections 8.4 and
//! 9.4): the component is compiled for the first platform; the others only answer for diagnostics.
//! `observability_differs` warns where a `bind:` is `exact` on one platform and not on another
//! (partial, none, set_only, constant, or no fact). Platforms where the tag or the property does not
//! exist are left out (the vocabularies differ: compile such a component per platform).

use crate::diag::Diagnostics;
use crate::estree::Spanned;
use crate::semantics::{PlatformSemantics, codes, spec::ObsClass};
use crate::svelte::ast::{AttributeLike, Root};
use crate::svelte::scope::walk::{NodeRef, fragment_node_ref};

pub fn check(root: &Root, platforms: &[&dyn PlatformSemantics], diags: &mut Diagnostics) {
    if platforms.len() < 2 {
        return;
    }
    let mut stack: Vec<NodeRef<'_>> = root.fragment.nodes.iter().map(fragment_node_ref).collect();
    while let Some(n) = stack.pop() {
        if let NodeRef::RegularElement(el) = n {
            for a in &el.attributes {
                let AttributeLike::BindDirective(d) = a else { continue };
                if d.name == "this" || !d.modifiers.is_empty() {
                    continue; // `|nonreactive` reads once on every platform
                }
                let mut seen: Vec<(String, Option<crate::semantics::Observability>)> = Vec::new();
                for p in platforms {
                    let Ok(er) = p.resolve_element(&el.name) else { continue };
                    let prop = match p.plan_binding(&er, &d.name) {
                        Ok(plan) => plan.prop,
                        Err(_) => d.name.clone(),
                    };
                    let Ok(spec) = p.property(&er, &prop) else { continue };
                    seen.push((p.backend().to_string(), p.observability(&er, spec)));
                }
                let exact = |o: &Option<crate::semantics::Observability>| o.as_ref().is_some_and(|o| o.class == ObsClass::Exact);
                if !(seen.iter().any(|(_, o)| exact(o)) && seen.iter().any(|(_, o)| !exact(o))) {
                    continue;
                }
                let parts: Vec<String> = seen
                    .iter()
                    .map(|(id, o)| match o {
                        Some(o) => format!("{} on {id}", o.describe()),
                        None => format!("unknown on {id} (no observe fact)"),
                    })
                    .collect();
                diags
                    .warning(codes::OBSERVABILITY_DIFFERS, (d.span().start, d.span().end), format!("bind:{} on <{}>: observability differs across platforms: {}", d.name, el.name, parts.join("; ")))
                    .help("where it is not exact the bound value can go stale; read it explicitly there, or use `|nonreactive`");
            }
        }
        stack.extend(n.children().into_iter().rev());
    }
}
