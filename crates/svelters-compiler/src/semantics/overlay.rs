//! [`Overlay`]: a [`PlatformSemantics`] that answers from an inner platform (usually a
//! [`SpecBackend`](super::SpecBackend)) except for the answers overridden in Rust (REACTIVITY.md
//! section 9.4): a platform before its metadata pipeline exists, a renderer without retained
//! objects, conformance doubles in tests. It replaces nothing in the spec backend; observability
//! overrides that are data belong in `metadata/overrides/<tk>/observe`, not here.

use super::spec::{MethodSpec, PropertySpec, StylePropSpec};
use super::*;
use std::collections::HashMap;

pub struct Overlay<P: PlatformSemantics> {
    inner: P,
    id: Option<String>,
    /// (tag, JS prop) -> observability (`None`: unknown).
    observe: HashMap<(String, String), Option<Observability>>,
}

impl<P: PlatformSemantics> Overlay<P> {
    pub fn new(inner: P) -> Self {
        Overlay { inner, id: None, observe: HashMap::new() }
    }

    /// Answer as another platform id (diagnostics and the portability mode).
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Override the observability of `prop` (JS name) on elements `tag`; `None` makes it unknown.
    pub fn with_observability(mut self, tag: &str, prop: &str, o: Option<Observability>) -> Self {
        self.observe.insert((tag.to_string(), prop.to_string()), o);
        self
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }
}

impl<P: PlatformSemantics> PlatformSemantics for Overlay<P> {
    fn backend(&self) -> &str {
        self.id.as_deref().unwrap_or_else(|| self.inner.backend())
    }
    fn framework(&self) -> &str {
        self.inner.framework()
    }
    fn row_outlet(&self) -> Option<&str> {
        self.inner.row_outlet()
    }
    fn resolve_element(&self, tag: &str) -> Result<ElementRef, Unresolved> {
        self.inner.resolve_element(tag)
    }
    fn source_loc(&self, el: &ElementRef) -> bool {
        self.inner.source_loc(el)
    }
    fn source_locs(&self) -> bool {
        self.inner.source_locs()
    }
    fn lifetime(&self, el: &ElementRef) -> LifetimePlan {
        self.inner.lifetime(el)
    }
    fn is_a(&self, el: &ElementRef, kind_or_class: &str) -> bool {
        self.inner.is_a(el, kind_or_class)
    }
    fn property(&self, el: &ElementRef, attr: &str) -> Result<&PropertySpec, Unresolved> {
        self.inner.property(el, attr)
    }
    fn writable_property(&self, el: &ElementRef, attr: &str) -> Result<&PropertySpec, Unresolved> {
        self.inner.writable_property(el, attr)
    }
    fn event(&self, el: &ElementRef, attr: &str) -> Result<EventSpec, Unresolved> {
        self.inner.event(el, attr)
    }
    fn plan_binding(&self, el: &ElementRef, prop: &str) -> Result<BindingPlan, Unresolved> {
        self.inner.plan_binding(el, prop)
    }
    fn place_child(&self, parent: &ElementRef, child: &ElementRef, requested_slot: Option<&str>) -> Result<SlotPlan, Unresolved> {
        self.inner.place_child(parent, child, requested_slot)
    }
    fn attaches_to(&self, child: &ElementRef) -> Option<&str> {
        self.inner.attaches_to(child)
    }
    fn text_content(&self, el: &ElementRef) -> Option<&PropertySpec> {
        self.inner.text_content(el)
    }
    fn style_property(&self, name: &str) -> Option<&StylePropSpec> {
        self.inner.style_property(name)
    }
    fn is_setter_property(&self, name: &str) -> bool {
        self.inner.is_setter_property(name)
    }
    fn style_engine_in_backend(&self) -> bool {
        self.inner.style_engine_in_backend()
    }
    fn style_declaration(&self, el: &ElementRef, parent: Option<&ElementRef>, name: &str, value: &[ValueToken]) -> Result<Option<StyleLowering>, Unresolved> {
        self.inner.style_declaration(el, parent, name, value)
    }
    fn coerce_static(&self, prop: &PropertySpec, value: Option<&str>) -> Result<StaticValue, String> {
        self.inner.coerce_static(prop, value)
    }
    fn text_node(&self) -> Option<(ElementRef, String)> {
        self.inner.text_node()
    }

    fn inline_text(&self) -> Option<(ElementRef, String)> {
        self.inner.inline_text()
    }
    fn child_attr(&self, parent: &ElementRef, slot: &str, name: &str) -> Result<ChildAttrPlan, Unresolved> {
        self.inner.child_attr(parent, slot, name)
    }
    fn coerce_child_attr(&self, plan: &ChildAttrPlan, value: Option<&str>) -> Result<StaticValue, String> {
        self.inner.coerce_child_attr(plan, value)
    }
    fn method(&self, el: &ElementRef, name: &str) -> Result<&MethodSpec, Unresolved> {
        self.inner.method(el, name)
    }
    fn html_syntax(&self) -> bool {
        self.inner.html_syntax()
    }
    fn observability(&self, el: &ElementRef, prop: &PropertySpec) -> Option<Observability> {
        match self.observe.get(&(el.tag.clone(), prop.js.clone())) {
            Some(o) => o.clone(),
            None => self.inner.observability(el, prop),
        }
    }
    fn bind_modifiers(&self) -> &[&'static str] {
        self.inner.bind_modifiers()
    }
    fn native_type(&self, module: &str, export: &str) -> Option<TypeRef> {
        self.inner.native_type(module, export)
    }
    fn bind_source(&self, module: &str, export: &str, lowering: spec::BindSourceLowering) -> Option<&spec::BindSourceSpec> {
        self.inner.bind_source(module, export, lowering)
    }
    fn type_property(&self, ty: &TypeRef, prop: &str) -> Option<TypeProp> {
        self.inner.type_property(ty, prop)
    }
    fn capabilities(&self) -> &[String] {
        self.inner.capabilities()
    }
}
