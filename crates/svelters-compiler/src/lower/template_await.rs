//! `{await p}` in the template (Svelte's experimental async template expressions) is not supported by
//! the native compiler: without this check it compiled into an `await` inside a non-async effect
//! function, which is invalid JavaScript (RT1 open item, fixed in FU3). An `await` inside a function
//! of the template (an `async` event handler) is ordinary JavaScript and stays allowed, and so does
//! the `{#await p}` block.

use crate::diag::Diagnostics;
use crate::svelte::ast::Root;
use crate::svelte::scope::walk::{NodeRef, fragment_node_ref};

pub fn check(root: &Root, diags: &mut Diagnostics) {
    for n in &root.fragment.nodes {
        walk(fragment_node_ref(n), diags);
    }
}

fn walk(node: NodeRef<'_>, diags: &mut Diagnostics) {
    if node.is_function() {
        return;
    }
    if let NodeRef::AwaitExpression(_) = node {
        let s = node.span();
        diags
            .error("unsupported_feature", (s.start, s.end), "`await` in a template expression is not supported by the native compiler yet")
            .help("await in the script (`$effect`, an event handler) and keep the result in `$state`, or use an `{#await p}` block");
        return;
    }
    for c in node.children() {
        walk(c, diags);
    }
}
