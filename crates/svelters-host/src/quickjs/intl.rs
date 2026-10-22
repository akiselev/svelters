//! IN1: the natives behind `Intl` (svelters-intl over ICU4X) and the JS shim that installs it
//! (`intl.js`). Options cross as the JSON the shim validated; tags are canonical.

use rquickjs::{Array, Ctx, Exception, Function, Object, Value};
use serde_json::Value as J;
use svelters_intl as intl;
use svelters_intl::{IntlError, Locale};

type Res<T> = rquickjs::Result<T>;

const SHIM: &str = include_str!("../intl.js");

fn throw<T>(ctx: &Ctx<'_>, e: IntlError) -> Res<T> {
    Err(match e {
        IntlError::Tag(_) | IntlError::Range(_) => Exception::throw_range(ctx, &e.to_string()),
        _ => Exception::throw_message(ctx, &e.to_string()),
    })
}

fn ok<T>(ctx: &Ctx<'_>, r: Result<T, IntlError>) -> Res<T> {
    r.or_else(|e| throw(ctx, e))
}

fn locale(ctx: &Ctx<'_>, tag: &str) -> Res<Locale> {
    ok(ctx, Locale::parse(tag))
}

fn opts(ctx: &Ctx<'_>, json: &str) -> Res<serde_json::Map<String, J>> {
    match serde_json::from_str::<J>(json) {
        Ok(J::Object(m)) => Ok(m),
        _ => Err(Exception::throw_type(ctx, "Intl: invalid options")),
    }
}

fn s<'a>(m: &'a serde_json::Map<String, J>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(J::as_str)
}

fn u8_of(m: &serde_json::Map<String, J>, k: &str) -> Option<u8> {
    m.get(k).and_then(J::as_f64).map(|v| v as u8)
}

fn number_options(m: &serde_json::Map<String, J>) -> intl::NumberOptions {
    use intl::{CurrencyDisplay, Grouping, Notation, NumberStyle, SignDisplay};
    let mut o = intl::NumberOptions::default();
    o.style = match s(m, "style") {
        Some("percent") => NumberStyle::Percent,
        Some("currency") => NumberStyle::Currency,
        _ => NumberStyle::Decimal,
    };
    o.currency = s(m, "currency").map(String::from);
    o.currency_display = match s(m, "currencyDisplay") {
        Some("narrowSymbol") => CurrencyDisplay::NarrowSymbol,
        Some("code") => CurrencyDisplay::Code,
        Some("name") => CurrencyDisplay::Name,
        _ => CurrencyDisplay::Symbol,
    };
    o.minimum_integer_digits = u8_of(m, "minimumIntegerDigits").unwrap_or(1);
    o.minimum_fraction_digits = u8_of(m, "minimumFractionDigits");
    o.maximum_fraction_digits = u8_of(m, "maximumFractionDigits");
    o.minimum_significant_digits = u8_of(m, "minimumSignificantDigits");
    o.maximum_significant_digits = u8_of(m, "maximumSignificantDigits");
    o.use_grouping = match m.get("useGrouping") {
        Some(J::Bool(false)) => Some(Grouping::Off),
        Some(J::String(g)) if g == "always" => Some(Grouping::Always),
        Some(J::String(g)) if g == "min2" => Some(Grouping::Min2),
        Some(J::String(g)) if g == "auto" => Some(Grouping::Auto),
        _ => None,
    };
    o.sign_display = match s(m, "signDisplay") {
        Some("never") => SignDisplay::Never,
        Some("always") => SignDisplay::Always,
        Some("exceptZero") => SignDisplay::ExceptZero,
        Some("negative") => SignDisplay::Negative,
        _ => SignDisplay::Auto,
    };
    o.notation = match (s(m, "notation"), s(m, "compactDisplay")) {
        (Some("compact"), Some("long")) => Notation::CompactLong,
        (Some("compact"), _) => Notation::CompactShort,
        _ => Notation::Standard,
    };
    o
}

fn date_options(m: &serde_json::Map<String, J>) -> intl::DateOptions {
    use intl::{DateStyle, HourCycle, MonthStyle, NumericStyle, TextStyle, TimeZoneNameStyle};
    let style = |k| match s(m, k) {
        Some("full") => Some(DateStyle::Full),
        Some("long") => Some(DateStyle::Long),
        Some("medium") => Some(DateStyle::Medium),
        Some("short") => Some(DateStyle::Short),
        _ => None,
    };
    let text = |k| match s(m, k) {
        Some("narrow") => Some(TextStyle::Narrow),
        Some("short") => Some(TextStyle::Short),
        Some("long") => Some(TextStyle::Long),
        _ => None,
    };
    let num = |k| match s(m, k) {
        Some("numeric") => Some(NumericStyle::Numeric),
        Some("2-digit") => Some(NumericStyle::TwoDigit),
        _ => None,
    };
    let mut o = intl::DateOptions::default();
    o.date_style = style("dateStyle");
    o.time_style = style("timeStyle");
    o.weekday = text("weekday");
    o.era = text("era");
    o.year = num("year");
    o.month = match s(m, "month") {
        Some("numeric") => Some(MonthStyle::Numeric),
        Some("2-digit") => Some(MonthStyle::TwoDigit),
        Some("narrow") => Some(MonthStyle::Narrow),
        Some("short") => Some(MonthStyle::Short),
        Some("long") => Some(MonthStyle::Long),
        _ => None,
    };
    o.day = num("day");
    o.hour = num("hour");
    o.minute = num("minute");
    o.second = num("second");
    o.fractional_second_digits = u8_of(m, "fractionalSecondDigits");
    o.time_zone_name = match s(m, "timeZoneName") {
        Some("short") => Some(TimeZoneNameStyle::Short),
        Some("long") => Some(TimeZoneNameStyle::Long),
        Some("shortOffset") => Some(TimeZoneNameStyle::ShortOffset),
        Some("longOffset") => Some(TimeZoneNameStyle::LongOffset),
        Some("shortGeneric") => Some(TimeZoneNameStyle::ShortGeneric),
        Some("longGeneric") => Some(TimeZoneNameStyle::LongGeneric),
        _ => None,
    };
    o.hour_cycle = match s(m, "hourCycle") {
        Some("h11") => Some(HourCycle::H11),
        Some("h12") => Some(HourCycle::H12),
        Some("h23") | Some("h24") => Some(HourCycle::H23),
        _ => None,
    };
    o
}

fn collator_options(m: &serde_json::Map<String, J>) -> intl::CollatorOptions {
    use intl::{CaseFirst, Sensitivity};
    let mut o = intl::CollatorOptions::default();
    o.sensitivity = match s(m, "sensitivity") {
        Some("base") => Sensitivity::Base,
        Some("accent") => Sensitivity::Accent,
        Some("case") => Sensitivity::Case,
        _ => Sensitivity::Variant,
    };
    o.numeric = m.get("numeric").and_then(J::as_bool).unwrap_or(false);
    o.ignore_punctuation = m.get("ignorePunctuation").and_then(J::as_bool).unwrap_or(false);
    o.case_first = match s(m, "caseFirst") {
        Some("upper") => Some(CaseFirst::Upper),
        Some("lower") => Some(CaseFirst::Lower),
        Some("false") => Some(CaseFirst::Off),
        _ => None,
    };
    o
}

fn parts_array<'js>(ctx: &Ctx<'js>, parts: Vec<intl::Part>) -> Res<Value<'js>> {
    let arr = Array::new(ctx.clone())?;
    for (i, p) in parts.into_iter().enumerate() {
        let o = Object::new(ctx.clone())?;
        o.set("type", p.kind)?;
        o.set("value", p.value)?;
        arr.set(i, o)?;
    }
    Ok(arr.into_value())
}

/// The IANA name of the host's zone (`TZ`, else the `/etc/localtime` link), or "" if unknown.
fn local_time_zone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if !tz.is_empty() && !tz.starts_with('/') {
            return tz.to_string();
        }
    }
    #[cfg(unix)]
    if let Ok(p) = std::fs::read_link("/etc/localtime") {
        let p = p.to_string_lossy();
        if let Some(i) = p.find("zoneinfo/") {
            return p[i + "zoneinfo/".len()..].to_string();
        }
    }
    String::new()
}

/// Installs `Intl`, the locale-sensitive prototype methods and `__svelters_intl`.
pub(super) fn install<'js>(ctx: &Ctx<'js>) -> Res<()> {
    let n = Object::new(ctx.clone())?;
    n.set("current", Function::new(ctx.clone(), || intl::current().tag().to_string())?)?;
    n.set(
        "canonicalize",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String| -> Res<String> { ok(&ctx, intl::canonicalize(&tag)) })?,
    )?;
    n.set(
        "resolve",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tags: Vec<String>| -> Res<String> {
            let refs: Vec<&str> = tags.iter().map(String::as_str).collect();
            ok(&ctx, intl::resolve(&refs)).map(|l| l.tag().to_string())
        })?,
    )?;
    n.set(
        "supported",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String| -> Res<bool> { Ok(locale(&ctx, &tag)?.is_supported()) })?,
    )?;
    n.set(
        "number",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String, json: String, x: f64, parts: bool| -> Res<Value<'js>> {
            let l = locale(&ctx, &tag)?;
            let o = number_options(&opts(&ctx, &json)?);
            let ps = ok(&ctx, intl::format_number_to_parts(&l, x, &o))?;
            if parts {
                parts_array(&ctx, ps)
            } else {
                let s: String = ps.iter().map(|p| p.value.as_str()).collect();
                rquickjs::String::from_str(ctx.clone(), &s).map(|s| s.into_value())
            }
        })?,
    )?;
    n.set(
        "numberDigits",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, json: String| -> Res<Vec<u8>> {
            let o = number_options(&opts(&ctx, &json)?);
            Ok(match ok(&ctx, o.resolved_fraction_digits())? {
                Some((min, max)) => vec![min, max],
                None => vec![0, 0],
            })
        })?,
    )?;
    n.set(
        "plural",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String, ordinal: bool, min: f64, max: f64, x: f64| -> Res<String> {
            let l = locale(&ctx, &tag)?;
            let mut o = if ordinal { intl::PluralOptions::ordinal() } else { intl::PluralOptions::cardinal() };
            o.minimum_fraction_digits = (min >= 0.0).then_some(min as u8);
            o.maximum_fraction_digits = (max >= 0.0).then_some(max as u8);
            ok(&ctx, intl::plural_category(&l, x, &o)).map(String::from)
        })?,
    )?;
    n.set(
        "pluralCategories",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String, ordinal: bool| -> Res<Vec<String>> {
            let l = locale(&ctx, &tag)?;
            ok(&ctx, intl::plural_categories(&l, ordinal)).map(|v| v.into_iter().map(String::from).collect())
        })?,
    )?;
    n.set(
        "compare",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, tag: String, json: String, a: String, b: String| -> Res<i32> {
            let l = locale(&ctx, &tag)?;
            let o = collator_options(&opts(&ctx, &json)?);
            ok(&ctx, intl::compare(&l, &a, &b, &o)).map(|o| o as i32)
        })?,
    )?;
    n.set(
        "date",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, tag: String, json: String, ms: f64, utc: bool, offset_s: f64, parts: bool| -> Res<Value<'js>> {
                let l = locale(&ctx, &tag)?;
                let o = date_options(&opts(&ctx, &json)?);
                let zone = if utc { intl::Zone::Utc } else { intl::Zone::Offset(offset_s as i32) };
                let ps = ok(&ctx, intl::format_date_to_parts(&l, ms, zone, &o))?;
                if parts {
                    parts_array(&ctx, ps)
                } else {
                    let s: String = ps.iter().map(|p| p.value.as_str()).collect();
                    rquickjs::String::from_str(ctx.clone(), &s).map(|s| s.into_value())
                }
            },
        )?,
    )?;
    n.set("localTimeZone", Function::new(ctx.clone(), local_time_zone)?)?;
    let shim: Function = ctx.eval(SHIM)?;
    shim.call::<_, ()>((n, ctx.globals()))
}

/// Tells JS the host changed the locale (`__svelters_intl` listeners, default formatters).
pub(super) fn changed(ctx: &Ctx<'_>, tag: &str) -> Res<()> {
    let api: Value = ctx.globals().get("__svelters_intl")?;
    let Some(api) = api.as_object() else { return Ok(()) };
    let f: Function = api.get("_changed")?;
    f.call::<_, ()>((tag,))
}
