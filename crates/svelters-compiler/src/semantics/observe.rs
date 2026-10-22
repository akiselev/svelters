//! Observability and the `bind:` lowering policy of milestone R2 (plans/research/REACTIVITY.md
//! sections 8.2-8.4, decisions 13 #4-#8): which runtime helper a `bind:` on a native element lowers
//! to, from the property's observe fact on the platform, and the diagnostics that go with it.

use super::spec::{ObsClass, ObsTiming, ObsVerified, ObserveSpec, ValueCost};
use super::{ElementRef, PlatformSemantics, Unresolved, codes};

/// Whether and how a native change of a property is announced on one platform (the compiler's
/// view of an `observe` fact; the mechanism string is for messages only).
#[derive(Debug, Clone, PartialEq)]
pub struct Observability {
    pub class: ObsClass,
    pub missed: Vec<String>,
    pub coarse: bool,
    pub value_cost: ValueCost,
    pub timing: ObsTiming,
    pub mechanism: String,
    /// Verification strength of the fact (OV1): only `Delivery` lets an `exact` `bind:` lower onto
    /// observation without a warning.
    pub verified: ObsVerified,
    /// The fact's name for messages (`Gtk:Entry:text`); empty when unknown.
    pub fact: String,
}

impl From<&ObserveSpec> for Observability {
    fn from(o: &ObserveSpec) -> Self {
        Observability {
            class: o.class,
            missed: o.missed.clone(),
            coarse: o.coarse,
            value_cost: o.value_cost,
            timing: o.timing,
            mechanism: o.mechanism.clone(),
            verified: o.verified,
            fact: o.fact.clone(),
        }
    }
}

/// The spec's word for a class (`exact`, `set_only`, ...).
pub fn class_name(c: ObsClass) -> &'static str {
    match c {
        ObsClass::Constant => "constant",
        ObsClass::SetOnly => "set_only",
        ObsClass::Exact => "exact",
        ObsClass::Partial => "partial",
        ObsClass::None => "none",
    }
}

/// A missed cause (`metadata/observe` `missed`) in words.
pub fn cause_text(c: &str) -> &str {
    match c {
        "user_edit" => "user edits",
        "programmatic" => "programmatic changes",
        "cross_prop" => "side effects of setting other properties",
        "parent" => "changes of the parent",
        "layout" => "layout",
        "wm" => "the window manager",
        "async" => "asynchronous changes",
        other => other,
    }
}

impl Observability {
    /// `exact (notify)`, `partial, misses user edits (notify)`, `none`, ...
    pub fn describe(&self) -> String {
        let mut s = class_name(self.class).to_string();
        if self.class == ObsClass::Partial && !self.missed.is_empty() {
            s.push_str(&format!(", misses {}", self.missed_text()));
        }
        if !self.mechanism.is_empty() {
            s.push_str(&format!(" ({})", self.mechanism));
        }
        s
    }

    /// The missed causes in words, joined (`user edits and layout`).
    pub fn missed_text(&self) -> String {
        let w: Vec<&str> = self.missed.iter().map(|c| cause_text(c)).collect();
        match w.as_slice() {
            [] => "some changes".into(),
            [one] => one.to_string(),
            [init @ .., last] => format!("{} and {last}", init.join(", ")),
        }
    }
}

/// What a native `bind:` lowers to (section 8.3).
#[derive(Debug, Clone, PartialEq)]
pub enum BindLowering {
    /// `$n.bind_prop(node, prop, get, set[, signal, arg])`: the write effect plus an observation of
    /// the prop (`exact`, or `partial` without a signal plan). `fallback` is the spec's signal plan,
    /// used at run time only when the host reports it cannot observe the prop yet (a backend
    /// without the mechanism: Qt before R3, AppKit/WinUI before R4).
    Prop { fallback: Option<(String, i64)> },
    /// `$n.bind_readonly(node, prop, set)`: a read-only observable prop, planned from the facts alone.
    Readonly,
    /// `$n.bind_once(node, prop, get, set, readonly)`: `|nonreactive` (section 8.2).
    Once { readonly: bool },
    /// `$n.bind(node, prop, signal, arg, get, set)`: the spec's signal plan, kept where the prop is not
    /// `exact` on this platform, or has no observe fact (decision 13 #8).
    Signal { signal: String, arg: i64 },
    /// CX3 `bind:prop|commit`: `$n.bind_commit(node, prop, get, set, signal, arg)`. State -> native
    /// as usual; native -> state only when the kind's commit event `signal` fires, with its
    /// `args[arg]`. No observation, so edits between commits run no JS.
    Commit { signal: String, arg: i64 },
}

/// The plan of one `bind:`: the JS prop name, the lowering, the fact it came from and warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct BindPlan {
    pub prop: String,
    pub lowering: BindLowering,
    pub observability: Option<Observability>,
    /// `bind_partially_observed`, `bind_observation_unverified`, `nonreactive_redundant` (errors under
    /// `--strict-native`).
    pub warnings: Vec<Unresolved>,
}

/// CX3: the plan of `bind:<name>|commit`: the kind's binding for `name` must declare a commit
/// event (`bindings.<prop>.commit` in the spec: egui's `commit` signal). Observability does not
/// matter: the commit event carries the value. A kind without one is `bind_commit_unsupported`.
pub fn plan_bind_commit<S: PlatformSemantics + ?Sized>(sem: &S, el: &ElementRef, name: &str) -> Result<BindPlan, Unresolved> {
    let platform = sem.backend().to_string();
    let tag = &el.tag;
    let plan = match sem.plan_binding(el, name) {
        Ok(p) => Some(p),
        Err(u) if matches!(u.code, codes::UNKNOWN_PROPERTY | codes::READ_ONLY_PROPERTY) => return Err(u),
        Err(_) => None,
    };
    let prop_name = plan.as_ref().map_or(name.to_string(), |p| p.prop.clone());
    let p = sem.property(el, &prop_name)?;
    let js = p.js.clone();
    if !p.readable || !p.writable {
        return Err(Unresolved::new(codes::READ_ONLY_PROPERTY, format!("bind:{name}|commit: `{js}` of <{tag}> is not readable and writable on {platform}"))
            .help(format!("set it one way with `{js}={{...}}`")));
    }
    let Some((signal, arg)) = plan.and_then(|p| p.commit) else {
        return Err(Unresolved::new(
            codes::BIND_COMMIT_UNSUPPORTED,
            format!("bind:{name}|commit on <{tag}>: <{tag}> has no commit event for `{js}` on {platform}, so there is no moment to commit at"),
        )
        .help(format!("bind it without `|commit` (every change), or set `{js}` one way and write your state from the kind's own event")));
    };
    let obs = sem.observability(el, p);
    Ok(BindPlan { prop: js, lowering: BindLowering::Commit { signal, arg: arg as i64 }, observability: obs, warnings: Vec::new() })
}

/// The `bind:<name>` policy (sections 8.2-8.4, decisions 13 #4-#9), shared by every
/// [`PlatformSemantics`]: it only asks the platform for the property, its observability and the
/// declared signal plan, so a changed observe fact just changes the output.
pub fn plan_bind<S: PlatformSemantics + ?Sized>(sem: &S, el: &ElementRef, name: &str, nonreactive: bool) -> Result<BindPlan, Unresolved> {
    let platform = sem.backend().to_string();
    // the declared signal plan (`bindings` annotations), looked up by binding or property name
    let signal = match sem.plan_binding(el, name) {
        Ok(p) => Some(p),
        Err(u) if u.code == codes::UNKNOWN_PROPERTY => return Err(u),
        Err(_) => None,
    };
    let prop_name = signal.as_ref().map_or(name.to_string(), |p| p.prop.clone());
    let p = sem.property(el, &prop_name)?;
    let js = p.js.clone();
    let obs = sem.observability(el, p);
    let tag = &el.tag;
    if !p.readable {
        return Err(Unresolved::new(codes::READ_ONLY_PROPERTY, format!("bind:{name}: `{js}` of <{tag}> is write-only on {platform}"))
            .help(format!("set it one way with `{js}={{...}}`")));
    }
    let class = obs.as_ref().map(|o| o.class);
    let mut warnings = Vec::new();
    if nonreactive {
        match class {
            Some(c @ (ObsClass::SetOnly | ObsClass::Constant)) => {
                let why = if c == ObsClass::SetOnly { "only your own writes change it" } else { "it never changes after construction" };
                warnings.push(
                    Unresolved::new(
                        codes::NONREACTIVE_REDUNDANT,
                        format!("bind:{name}|nonreactive on <{tag}>: `{js}` is {} on {platform}: {why}, so `|nonreactive` changes nothing", class_name(c)),
                    )
                    .help(format!("use `{js}={{...}}`")),
                );
            }
            _ => {}
        }
        return Ok(BindPlan { prop: js, lowering: BindLowering::Once { readonly: !p.writable }, observability: obs, warnings });
    }
    let sig = signal.as_ref().map(|s| (s.signal.clone(), s.arg_operand()));
    let lowering = match (class, &obs) {
        (Some(ObsClass::Exact), Some(o)) if o.verified == ObsVerified::Delivery => {
            if p.writable {
                BindLowering::Prop { fallback: sig }
            } else {
                BindLowering::Readonly
            }
        }
        // `exact`, but no generated row has seen a native change delivered (OV1): a silent prop would
        // leave the bound value stale, so a declared signal plan stays; without one the binding still
        // observes, with a warning naming the fact.
        (Some(ObsClass::Exact), Some(o)) => match sig {
            Some((signal, arg)) => BindLowering::Signal { signal, arg },
            None => {
                let what = match o.verified {
                    ObsVerified::Weak => "its test only checks that the observation installs",
                    _ => "it has no passing test",
                };
                let fact = if o.fact.is_empty() { String::new() } else { format!(" (fact {})", o.fact) };
                warnings.push(
                    Unresolved::new(
                        codes::BIND_OBSERVATION_UNVERIFIED,
                        format!(
                            "bind:{name} on <{tag}>: `{js}` is exact on {platform}{fact}, but no native change has been seen delivered ({what}), so the bound value could go stale"
                        ),
                    )
                    .help("read it explicitly where it matters (node.getProp), or opt out with `|nonreactive`; verifying the fact (metadata/observe) removes this warning"),
                );
                if p.writable { BindLowering::Prop { fallback: None } } else { BindLowering::Readonly }
            }
        },
        (Some(ObsClass::Partial), Some(o)) => {
            warnings.push(
                Unresolved::new(
                    codes::BIND_PARTIALLY_OBSERVED,
                    format!("bind:{name} on <{tag}>: `{js}` is only partially observed on {platform}: {} are not announced, so the bound value can go stale after them", o.missed_text()),
                )
                .help("read it explicitly where it matters (node.getProp), or opt out with `|nonreactive`"),
            );
            match sig {
                Some((signal, arg)) => BindLowering::Signal { signal, arg },
                None if p.writable => BindLowering::Prop { fallback: None },
                None => BindLowering::Readonly,
            }
        }
        (Some(ObsClass::SetOnly), _) if p.writable => {
            return Err(Unresolved::new(
                codes::BIND_SET_ONLY,
                format!("bind:{name} on <{tag}>: `{js}` is set_only on {platform}: nothing but your code changes it, so there is nothing to bind back"),
            )
            .help(format!("use `{js}={{...}}`")));
        }
        (Some(c @ (ObsClass::None | ObsClass::Constant | ObsClass::SetOnly)), _) => {
            let why = match c {
                ObsClass::Constant => "it never changes after construction".to_string(),
                _ => "its native changes are not announced".to_string(),
            };
            return Err(Unresolved::new(codes::BIND_NOT_OBSERVABLE, format!("bind:{name} on <{tag}>: `{js}` is {} on {platform}: {why}", class_name(c)))
                .help(if c == ObsClass::Constant {
                    format!("set it once with `{js}={{...}}`")
                } else {
                    format!("read it when you need it (`bind:{name}|nonreactive` or node.getProp), or listen to an event")
                }));
        }
        (None, _) | (Some(ObsClass::Partial | ObsClass::Exact), None) => match sig {
            // no observe fact: unknown, never lowered onto observation (R0)
            Some((signal, arg)) => BindLowering::Signal { signal, arg },
            None => {
                return Err(Unresolved::new(
                    codes::BIND_NO_PLAN,
                    format!("bind:{name} is not supported on <{tag}> ({platform}): whether native changes of `{js}` are announced is unknown (no observe fact in metadata/observe/{platform}.json) and no binding is declared"),
                )
                .help(format!("set the property one way and listen to an event instead, or read it once with `bind:{name}|nonreactive`")));
            }
        },
    };
    Ok(BindPlan { prop: js, lowering, observability: obs, warnings })
}
