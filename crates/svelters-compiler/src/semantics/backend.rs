//! [`SpecBackend`]: [`BackendSemantics`] answered from a generated `svelters-semantics/v0` spec.

use super::spec::*;
use super::*;
use std::path::Path;

const GTK_JSON: &str = include_str!("../../../../metadata/semantics/gtk.json");
const QT_JSON: &str = include_str!("../../../../metadata/semantics/qt.json");

#[derive(Debug, Clone)]
pub struct SpecBackend {
    spec: Spec,
}

impl SpecBackend {
    pub fn from_json(json: &str) -> Result<SpecBackend, String> {
        Ok(SpecBackend { spec: Spec::parse(json)? })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<SpecBackend, String> {
        let p = path.as_ref();
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        SpecBackend::from_json(&text)
    }

    /// The checked-in spec of a built-in backend (`gtk` or `qt`), embedded at build time.
    pub fn builtin(backend: &str) -> Result<SpecBackend, String> {
        match backend {
            "gtk" => SpecBackend::from_json(GTK_JSON),
            "qt" => SpecBackend::from_json(QT_JSON),
            other => Err(format!("no built-in semantic spec for backend {other:?} (known: gtk, qt)")),
        }
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    pub fn kind<'a>(&'a self, el: &ElementRef) -> &'a KindSpec {
        &self.spec.kinds[&el.kind]
    }

    /// The kinds of `el`'s chain that the spec defines (Qt chains also name plain classes).
    fn chain<'a>(&'a self, el: &ElementRef) -> Vec<(&'a str, &'a KindSpec)> {
        self.kind(el).chain.iter().filter_map(|id| self.spec.kinds.get(id).map(|k| (id.as_str(), k))).collect()
    }

    fn props<'a>(&'a self, el: &ElementRef) -> Vec<&'a PropertySpec> {
        self.chain(el).into_iter().flat_map(|(_, k)| k.props.values()).collect()
    }

    fn find_prop<'a>(&'a self, el: &ElementRef, name: &str) -> Option<&'a PropertySpec> {
        self.props(el).into_iter().find(|p| p.js == name || p.native == name)
    }

    fn find_signal<'a>(&'a self, el: &ElementRef, name: &str) -> Option<&'a SignalSpec> {
        self.chain(el).into_iter().flat_map(|(_, k)| k.signals.values()).find(|s| s.name == name || s.js_event == name)
    }

    fn slots<'a>(&'a self, el: &ElementRef) -> Vec<(&'a str, &'a str, &'a SlotSpec)> {
        self.chain(el)
            .into_iter()
            .filter_map(|(id, k)| k.children.as_ref().map(|c| (id, c)))
            .flat_map(|(id, c)| c.slots.iter().map(move |(n, s)| (id, n.as_str(), s)))
            .collect()
    }

    fn default_slot<'a>(&'a self, el: &ElementRef) -> Option<&'a str> {
        self.chain(el).into_iter().find_map(|(_, k)| k.children.as_ref().and_then(|c| c.default_slot.as_deref()))
    }

    fn only_annotated(&self) -> &'static str {
        if self.spec.backend == "qt" {
            " (only annotated properties exist on qt: metadata/overrides/qt)"
        } else {
            ""
        }
    }

    fn length(&self, decl: &str, value: f64, unit: Option<&str>) -> Result<f64, Unresolved> {
        let scale = match unit {
            None => 1.0,
            Some(u) => match self.spec.style.units.get(u) {
                Some(us) => us.scale,
                None => {
                    let known: Vec<String> = self.spec.style.units.keys().cloned().collect();
                    return Err(Unresolved::new(
                        codes::STYLE_BAD_VALUE,
                        format!("`{decl}`: unit `{u}` is not supported on {} (lengths are unitless or in {})", self.spec.backend, known.join(", ")),
                    )
                    .suggest(known.iter().map(|k| format!("{value}{k}")).collect()));
                }
            },
        };
        Ok(value * scale)
    }

    /// The native value of a setter / child-attr declaration (`prop`: the target property, whose
    /// range is checked; None for a child attribute).
    fn style_value(&self, decl: &str, p: &StylePropSpec, prop: Option<&PropertySpec>, value: &[ValueToken]) -> Result<StaticValue, Unresolved> {
        let bad = |msg: String| Unresolved::new(codes::STYLE_BAD_VALUE, format!("`{decl}`: {msg}"));
        let kind = p.value.ok_or_else(|| bad("the style table gives no value type".into()))?;
        let whole = |x: f64| -> Result<StaticValue, Unresolved> {
            if let Some(r) = prop.and_then(|pr| pr.range)
                && (x < r.min || x > r.max)
            {
                return Err(bad(format!("{x} is outside {}..{}", r.min, r.max)));
            }
            if x.fract() != 0.0 {
                return Err(bad(format!("takes whole pixels, {x} is not")));
            }
            Ok(StaticValue::Int(x as i64))
        };
        if kind == ValueKind::Insets {
            let mut n = Vec::new();
            for t in value {
                match t {
                    ValueToken::Number { value, unit } => n.push(self.length(decl, *value, unit.as_deref())?),
                    t => return Err(bad(format!("expected 1 to 4 lengths, got {}", token_text(t)))),
                }
            }
            if n.is_empty() || n.len() > 4 || n.iter().any(|x| x.fract() != 0.0) {
                return Err(bad("expected 1 to 4 whole lengths (top right bottom left, as CSS)".into()));
            }
            let (t, r) = (n[0], *n.get(1).unwrap_or(&n[0]));
            let (b, l) = (*n.get(2).unwrap_or(&t), *n.get(3).unwrap_or(&r));
            return Ok(StaticValue::Record(serde_json::json!({ "top": t as i64, "right": r as i64, "bottom": b as i64, "left": l as i64 })));
        }
        let one = match value {
            [t] => t,
            [] => return Err(bad("missing value".into())),
            _ => return Err(bad("expected a single value".into())),
        };
        Ok(match (kind, one) {
            (ValueKind::Enum, ValueToken::Ident(id)) => {
                let values = p.values.clone().unwrap_or_default();
                if !values.iter().any(|v| v == id) {
                    return Err(bad(format!("`{id}` is not one of {}", values.join(", "))).suggest(closest(id, values.iter().map(|s| s.as_str()))));
                }
                StaticValue::Enum(id.clone())
            }
            (ValueKind::Bool, ValueToken::Ident(id)) => match id.as_str() {
                "true" => StaticValue::Bool(true),
                "false" => StaticValue::Bool(false),
                _ => return Err(bad(format!("`{id}` is not true or false")).suggest(closest(id, ["true", "false"]))),
            },
            (ValueKind::Length, ValueToken::Number { value, unit }) => {
                let x = self.length(decl, *value, unit.as_deref())?;
                if prop.is_some_and(|pr| !pr.ty.is_integer()) { StaticValue::Float(x) } else { whole(x)? }
            }
            (ValueKind::Int, ValueToken::Number { value, unit: None }) => whole(*value)?,
            (ValueKind::Number, ValueToken::Number { value, unit: None }) => StaticValue::Float(*value),
            (ValueKind::String, ValueToken::String(t) | ValueToken::Ident(t)) => StaticValue::Str(t.clone()),
            (k, t) => return Err(bad(format!("expected {}, got {}", value_kind_name(k), token_text(t)))),
        })
    }
}

fn value_kind_name(k: ValueKind) -> &'static str {
    match k {
        ValueKind::Enum => "a keyword",
        ValueKind::Length => "a length (e.g. 8px)",
        ValueKind::Int => "an integer",
        ValueKind::Number => "a number",
        ValueKind::Bool => "true or false",
        ValueKind::Insets => "1 to 4 lengths",
        ValueKind::String => "a string",
    }
}

fn token_text(t: &ValueToken) -> String {
    match t {
        ValueToken::Ident(s) => format!("`{s}`"),
        ValueToken::Number { value, unit } => format!("`{value}{}`", unit.as_deref().unwrap_or("")),
        ValueToken::String(s) => format!("string {s:?}"),
        ValueToken::Color(c) => format!("color `{c}`"),
        ValueToken::Function { name, raw } => format!("`{name}({raw})`"),
    }
}

fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return i64::from_str_radix(h, 16).ok();
    }
    t.parse::<i64>().ok()
}

impl BackendSemantics for SpecBackend {
    fn backend(&self) -> &str {
        &self.spec.backend
    }

    fn framework(&self) -> &str {
        &self.spec.framework
    }

    fn resolve_element(&self, tag: &str) -> Result<ElementRef, Unresolved> {
        if let Some(kind) = self.spec.tags.get(tag) {
            return Ok(ElementRef { tag: tag.to_string(), kind: kind.clone() });
        }
        let mut u = Unresolved::new(
            codes::UNKNOWN_ELEMENT,
            format!("<{tag}> is not an element of the {} backend", self.spec.backend),
        )
        .suggest(closest(tag, self.spec.tags.keys().map(|s| s.as_str())));
        if let Some(h) = self.spec.element_hints.get(tag) {
            u = u.help(h.clone());
        }
        Err(u)
    }

    fn lifetime(&self, el: &ElementRef) -> LifetimePlan {
        let k = self.kind(el);
        LifetimePlan { lifetime: k.lifetime, present: k.present.clone() }
    }

    fn is_a(&self, el: &ElementRef, name: &str) -> bool {
        el.tag == name
            || self.kind(el).chain.iter().any(|c| c == name || self.spec.kinds.get(c).is_some_and(|k| k.native == name))
    }

    fn property(&self, el: &ElementRef, attr: &str) -> Result<&PropertySpec, Unresolved> {
        self.find_prop(el, attr).ok_or_else(|| {
            let names: Vec<&str> = self.props(el).into_iter().map(|p| p.js.as_str()).collect();
            Unresolved::new(
                codes::UNKNOWN_PROPERTY,
                format!("<{}> has no property `{attr}` on {}{}", el.tag, self.spec.backend, self.only_annotated()),
            )
            .suggest(closest(attr, names))
        })
    }

    fn event(&self, el: &ElementRef, attr: &str) -> Result<EventSpec, Unresolved> {
        let stripped = attr.strip_prefix("on").filter(|s| !s.is_empty()).unwrap_or(attr);
        let mk = |s: &SignalSpec, alias: Option<&str>| EventSpec {
            signal: s.name.clone(),
            owner: s.owner.clone(),
            args: s.args.iter().map(|a| (a.name.clone(), a.ty.clone())).collect(),
            handler_result: s.handler_result,
            sync_required: s.sync_required,
            disconnect_on_drop: s.disconnect_on_drop,
            sync: s.sync,
            phases: s.phases.clone(),
            via_alias: alias.map(str::to_string),
        };
        for name in [stripped, attr] {
            if let Some(target) = self.spec.event_aliases.get(name)
                && let Some(s) = self.find_signal(el, target)
            {
                return Ok(mk(s, Some(name)));
            }
            if let Some(s) = self.find_signal(el, name) {
                return Ok(mk(s, None));
            }
        }
        let mut names: Vec<&str> = self.chain(el).into_iter().flat_map(|(_, k)| k.signals.keys().map(|s| s.as_str())).collect();
        names.extend(self.spec.event_aliases.iter().filter(|(_, t)| self.find_signal(el, t).is_some()).map(|(a, _)| a.as_str()));
        Err(Unresolved::new(codes::UNKNOWN_EVENT, format!("<{}> has no event `{stripped}` on {}", el.tag, self.spec.backend))
            .suggest(closest(stripped, names).into_iter().map(|s| format!("on{s}")).collect()))
    }

    fn plan_binding(&self, el: &ElementRef, name: &str) -> Result<BindingPlan, Unresolved> {
        let chain = self.chain(el);
        let found = chain
            .iter()
            .find_map(|(_, k)| k.bindings.get(name))
            .or_else(|| {
                let js = self.find_prop(el, name).map(|p| p.js.as_str())?;
                chain.iter().flat_map(|(_, k)| k.bindings.values()).find(|b| b.property == js)
            });
        if let Some(b) = found {
            let p = self.find_prop(el, &b.property).expect("binding property is in the chain (generator-checked)");
            return Ok(BindingPlan {
                prop: p.js.clone(),
                prop_type: p.ty,
                signal: b.event.clone(),
                value: b.value,
                echo: b.echo,
                direction: b.direction,
                compare_before_set: b.compare_before_set,
                read_back: b.read_back,
                natively_normalizes: b.natively_normalizes,
                sync_signals_on_set: b.sync_signals_on_set.clone(),
            });
        }
        let prop = name;
        let p = self.property(el, prop)?;
        if !p.writable || !p.readable {
            return Err(Unresolved::new(
                codes::READ_ONLY_PROPERTY,
                format!("bind:{prop}: `{}` of <{}> is not readable and writable", p.js, el.tag),
            ));
        }
        let why = match &p.notify {
            Some(n) if self.find_signal(el, n).is_some() => format!(
                "`{n}` exists, but no binding is declared for `{}` (add a `bindings` annotation in metadata/overrides/{})",
                p.js, self.spec.backend
            ),
            Some(n) => format!(
                "its native change notification `{n}` is not exposed as a signal by the {} backend",
                self.spec.backend
            ),
            None => format!("{}::{} has no change notification", self.spec.kinds[&p.owner].native, p.native),
        };
        let bound: Vec<String> = self.chain(el).into_iter().flat_map(|(_, k)| k.bindings.keys().cloned()).collect();
        Err(Unresolved::new(codes::BIND_NO_PLAN, format!("bind:{prop} is not supported on <{}> ({}): {why}", el.tag, self.spec.backend))
            .suggest(bound)
            .help("set the property one way and listen to an event instead"))
    }

    fn place_child(&self, parent: &ElementRef, child: &ElementRef, requested: Option<&str>) -> Result<SlotPlan, Unresolved> {
        let all: Vec<(&str, &str, &SlotSpec)> = self.slots(parent);
        if all.is_empty() {
            return Err(Unresolved::new(
                codes::CHILDREN_NOT_ALLOWED,
                format!("<{}> takes no child elements on {}", parent.tag, self.spec.backend),
            ));
        }
        let default = self.default_slot(parent);
        let accepts = |s: &SlotSpec| s.accepts.iter().any(|a| self.is_a(child, a));
        let plan = |owner: &str, name: &str, s: &SlotSpec, routing| SlotPlan {
            slot: name.to_string(),
            is_default: Some(name) == default,
            routing,
            owner: owner.to_string(),
            many: s.many,
            order: s.order,
        };
        let names = || all.iter().map(|(_, n, _)| n.to_string()).collect::<Vec<_>>();
        let (owner, name, s, routing) = if let Some(req) = requested {
            match all.iter().find(|(_, n, _)| *n == req) {
                Some(&(o, n, s)) => (o, n, s, Routing::Explicit),
                None => {
                    let mut sug = closest(req, all.iter().map(|(_, n, _)| *n));
                    if sug.is_empty() {
                        sug = names();
                    }
                    return Err(Unresolved::new(
                        codes::UNKNOWN_SLOT,
                        format!("<{}> has no slot `{req}` on {} (slots: {})", parent.tag, self.spec.backend, names().join(", ")),
                    )
                    .suggest(sug));
                }
            }
        } else {
            let routed: Vec<_> = all.iter().filter(|(_, _, s)| s.route_by_kind && accepts(s)).collect();
            if let [only] = routed.as_slice() {
                let (o, n, s) = **only;
                (o, n, s, Routing::ByKind)
            } else {
                let Some(d) = default else {
                    return Err(Unresolved::new(
                        codes::NO_DEFAULT_SLOT,
                        format!("<{}> has no default slot; name one with slot=\"...\"", parent.tag),
                    )
                    .suggest(names()));
                };
                let &(o, n, s) = all.iter().find(|(_, n, _)| *n == d).expect("default slot is declared");
                (o, n, s, Routing::Default)
            }
        };
        if !accepts(s) {
            let ok: Vec<String> = all.iter().filter(|(_, _, s)| accepts(s)).map(|(_, n, _)| n.to_string()).collect();
            return Err(Unresolved::new(
                codes::SLOT_REJECTS_CHILD,
                format!(
                    "slot `{name}` of <{}> accepts {}, not <{}> ({})",
                    parent.tag,
                    s.accepts.join(" | "),
                    child.tag,
                    self.kind(child).native
                ),
            )
            .suggest(ok));
        }
        Ok(plan(owner, name, s, routing))
    }

    fn text_content(&self, el: &ElementRef) -> Option<&PropertySpec> {
        let name = self.kind(el).text_content.as_deref()?;
        self.find_prop(el, name)
    }

    fn style_property(&self, name: &str) -> Option<&StylePropSpec> {
        self.spec.style.properties.get(name)
    }

    fn style_declaration(&self, el: &ElementRef, parent: Option<&ElementRef>, name: &str, value: &[ValueToken]) -> Result<Option<StyleLowering>, Unresolved> {
        let Some(p) = self.spec.style.properties.get(name) else { return Ok(None) };
        let list = |tags: &[String]| tags.iter().map(|t| format!("<{t}>")).collect::<Vec<_>>().join(", ");
        match p.via {
            Via::Css => Ok(None),
            Via::Setter => {
                if !p.tags.contains(&el.tag) {
                    return Err(Unresolved::new(
                        codes::STYLE_NOT_APPLICABLE,
                        format!("`{name}` applies to {} on {}, not <{}>", list(&p.tags), self.spec.backend, el.tag),
                    ));
                }
                let target = p.prop.as_deref().unwrap_or_default();
                let prop = self.find_prop(el, target).ok_or_else(|| {
                    Unresolved::new(codes::STYLE_NOT_APPLICABLE, format!("`{name}` lowers to `{target}`, which <{}> does not have on {}{}", el.tag, self.spec.backend, self.only_annotated()))
                })?;
                let value = self.style_value(name, p, Some(prop), value)?;
                Ok(Some(StyleLowering::SetProp { prop: prop.js.clone(), value }))
            }
            Via::ChildAttr => {
                let attr = p.attr.clone().unwrap_or_default();
                match parent {
                    Some(par) if p.tags.contains(&par.tag) => {}
                    Some(par) => {
                        return Err(Unresolved::new(
                            codes::STYLE_NOT_APPLICABLE,
                            format!("`{name}` is a child attribute of {} on {}; <{}> is in a <{}>", list(&p.tags), self.spec.backend, el.tag, par.tag),
                        ))
                    }
                    None => {
                        return Err(Unresolved::new(
                            codes::STYLE_NOT_APPLICABLE,
                            format!("`{name}` is a child attribute of {} on {}; <{}> has no native parent here", list(&p.tags), self.spec.backend, el.tag),
                        ))
                    }
                }
                let value = self.style_value(name, p, None, value)?;
                Ok(Some(StyleLowering::ChildAttr { attr, value }))
            }
        }
    }

    fn text_node(&self) -> Option<(ElementRef, String)> {
        let t = self.spec.text_node.as_ref()?;
        let el = self.resolve_element(&t.kind).ok()?;
        let prop = self.find_prop(&el, &t.prop)?.js.clone();
        Some((el, prop))
    }

    fn html_syntax(&self) -> bool {
        self.spec.html_syntax
    }

    fn child_attr(&self, parent: &ElementRef, slot: &str, name: &str) -> Result<ChildAttrPlan, Unresolved> {
        let slots = self.slots(parent);
        let Some((_, _, s)) = slots.iter().find(|(_, n, _)| *n == slot) else {
            return Err(Unresolved::new(codes::UNKNOWN_SLOT, format!("<{}> has no slot `{slot}` on {}", parent.tag, self.spec.backend)));
        };
        if let Some(a) = s.attrs.get(name) {
            return Ok(ChildAttrPlan { name: name.to_string(), slot: slot.to_string(), spec: a.clone() });
        }
        let here: Vec<&str> = s.attrs.keys().map(|k| k.as_str()).collect();
        let elsewhere: Vec<String> =
            slots.iter().filter(|(_, n, sl)| *n != slot && sl.attrs.contains_key(name)).map(|(_, n, _)| n.to_string()).collect();
        let msg = if here.is_empty() {
            format!("slot `{slot}` of <{}> has no child attributes on {}", parent.tag, self.spec.backend)
        } else {
            format!("slot `{slot}` of <{}> has no child attribute `{name}` on {} (attributes: {})", parent.tag, self.spec.backend, here.join(", "))
        };
        let mut u = Unresolved::new(codes::UNKNOWN_CHILD_ATTR, msg).suggest(closest(name, here).into_iter().map(|n| format!("parent:{n}")).collect());
        if !elsewhere.is_empty() {
            u = u.help(format!("`{name}` belongs to slot {} of <{}>", elsewhere.join(", "), parent.tag));
        }
        Err(u)
    }

    fn coerce_child_attr(&self, plan: &ChildAttrPlan, value: Option<&str>) -> Result<StaticValue, String> {
        let name = &plan.name;
        let Some(s) = value else {
            return match plan.spec.ty {
                AttrType::Bool => Ok(StaticValue::Bool(true)),
                _ => Err(format!("`parent:{name}` needs a value (a bare attribute only sets bool attributes)")),
            };
        };
        match plan.spec.ty {
            AttrType::Bool => match s {
                "true" | "" => Ok(StaticValue::Bool(true)),
                "false" => Ok(StaticValue::Bool(false)),
                _ => Err(format!("`parent:{name}` is a bool: expected true or false, got {s:?}")),
            },
            AttrType::I32 | AttrType::U32 | AttrType::I64 => {
                let i = parse_int(s).ok_or_else(|| format!("`parent:{name}` is an integer, got {s:?}"))?;
                let (lo, hi) = match plan.spec.ty {
                    AttrType::I32 => (i32::MIN as i64, i32::MAX as i64),
                    AttrType::U32 => (0, u32::MAX as i64),
                    _ => (-9_007_199_254_740_991, 9_007_199_254_740_991),
                };
                if i < lo || i > hi {
                    return Err(format!("`parent:{name}`: {i} is outside {lo}..{hi}"));
                }
                Ok(StaticValue::Int(i))
            }
            AttrType::F64 => {
                let f: f64 = s.trim().parse().map_err(|_| format!("`parent:{name}` is a number, got {s:?}"))?;
                if !f.is_finite() {
                    return Err(format!("`parent:{name}`: {s:?} is not finite"));
                }
                Ok(StaticValue::Float(f))
            }
            AttrType::String => Ok(StaticValue::Str(s.to_string())),
            AttrType::Enum => {
                let vals = plan.spec.values.clone().unwrap_or_default();
                if vals.iter().any(|v| v == s) {
                    Ok(StaticValue::Enum(s.to_string()))
                } else {
                    let sug = closest(s, vals.iter().map(|v| v.as_str()));
                    let mean = sug.first().map(|x| format!("; did you mean {x:?}?")).unwrap_or_default();
                    Err(format!("`parent:{name}`: {s:?} is not one of {}{mean}", vals.join(", ")))
                }
            }
        }
    }

    fn method(&self, el: &ElementRef, name: &str) -> Result<&MethodSpec, Unresolved> {
        let chain = self.chain(el);
        if let Some(m) = chain.iter().find_map(|(_, k)| k.methods.get(name)) {
            return Ok(m);
        }
        let names: Vec<&str> = chain.iter().flat_map(|(_, k)| k.methods.keys().map(|s| s.as_str())).collect();
        let mut u = Unresolved::new(codes::UNKNOWN_METHOD, format!("<{}> has no annotated method `{name}` on {}", el.tag, self.spec.backend))
            .suggest(closest(name, names));
        if self.spec.method_lookup == "typelib" {
            u = u.help("other GIR methods are still callable through GIRepository, unchecked");
        }
        Err(u)
    }

    fn coerce_static(&self, prop: &PropertySpec, value: Option<&str>) -> Result<StaticValue, String> {
        let name = &prop.js;
        let Some(s) = value else {
            return match prop.ty {
                PropType::Bool => Ok(StaticValue::Bool(true)),
                _ => Err(format!("`{name}` needs a value (a bare attribute only sets bool properties)")),
            };
        };
        match prop.ty {
            PropType::Bool => match s {
                "true" | "" => Ok(StaticValue::Bool(true)),
                "false" => Ok(StaticValue::Bool(false)),
                _ => Err(format!("`{name}` is a bool: expected true or false, got {s:?}")),
            },
            PropType::I32 | PropType::U32 | PropType::I64 => {
                let i = parse_int(s).ok_or_else(|| format!("`{name}` is an integer, got {s:?}"))?;
                let (lo, hi) = match prop.ty {
                    PropType::I32 => (i32::MIN as f64, i32::MAX as f64),
                    PropType::U32 => (0.0, u32::MAX as f64),
                    _ => (-9.007_199_254_740_991e15, 9.007_199_254_740_991e15),
                };
                let (lo, hi) = prop.range.map_or((lo, hi), |r| (r.min.max(lo), r.max.min(hi)));
                if (i as f64) < lo || (i as f64) > hi {
                    return Err(format!("`{name}`: {i} is outside {lo}..{hi}"));
                }
                Ok(StaticValue::Int(i))
            }
            PropType::F64 => {
                let f: f64 = s.trim().parse().map_err(|_| format!("`{name}` is a number, got {s:?}"))?;
                if !f.is_finite() {
                    return Err(format!("`{name}`: {s:?} is not finite"));
                }
                if let Some(r) = prop.range
                    && (f < r.min || f > r.max)
                {
                    return Err(format!("`{name}`: {f} is outside {}..{}", r.min, r.max));
                }
                Ok(StaticValue::Float(f))
            }
            PropType::String => Ok(StaticValue::Str(s.to_string())),
            PropType::List if prop.of.as_deref() == Some("string") => {
                Ok(StaticValue::StrList(s.split_ascii_whitespace().map(str::to_string).collect()))
            }
            PropType::Enum => match &prop.values {
                Some(vals) if vals.iter().any(|v| v == s) => Ok(StaticValue::Enum(s.to_string())),
                Some(vals) => {
                    let sug = closest(s, vals.iter().map(|v| v.as_str()));
                    let mean = sug.first().map(|x| format!("; did you mean {x:?}?")).unwrap_or_default();
                    Err(format!("`{name}`: {s:?} is not one of {}{mean}", vals.join(", ")))
                }
                None => Ok(StaticValue::Enum(s.to_string())),
            },
            PropType::Flags => {
                let Some(values) = &prop.values else {
                    return Err(format!("`{name}`: flags of {} cannot be named statically", prop.enum_type.as_deref().unwrap_or("?")));
                };
                let mut nicks = Vec::new();
                let mut total = prop.bits.as_ref().map(|_| 0u64);
                for n in s.split(|c: char| c == '|' || c.is_ascii_whitespace()).filter(|n| !n.is_empty()) {
                    if !values.iter().any(|v| v == n) {
                        return Err(format!("`{name}`: {n:?} is not a flag (did you mean {:?}?)", closest(n, values.iter().map(|v| v.as_str()))));
                    }
                    if let (Some(t), Some(b)) = (total.as_mut(), prop.bits.as_ref().and_then(|b| b.get(n))) {
                        *t |= b;
                    }
                    nicks.push(n.to_string());
                }
                Ok(StaticValue::Flags { nicks, bits: total })
            }
            PropType::Object | PropType::Record | PropType::Boxed if prop.string_form => Ok(StaticValue::Str(s.to_string())),
            PropType::Object | PropType::List | PropType::Record | PropType::Boxed | PropType::Other => Err(format!(
                "`{name}` takes {} and cannot be set from a string attribute; pass an expression",
                prop.type_ref.as_deref().or(prop.of.as_deref()).unwrap_or("a native value")
            )),
        }
    }
}
