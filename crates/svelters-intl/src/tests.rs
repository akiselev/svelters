//! Expected strings are what V8 (Node 22, ICU 74+) prints for the same calls, except where noted.

use crate::*;
use std::cmp::Ordering;

fn l(tag: &str) -> Locale {
    Locale::parse(tag).unwrap()
}

fn num(tag: &str, x: f64, o: &NumberOptions) -> String {
    format_number(&l(tag), x, o).unwrap()
}

fn opts(f: impl FnOnce(&mut NumberOptions)) -> NumberOptions {
    let mut o = NumberOptions::default();
    f(&mut o);
    o
}

#[test]
fn data_has_the_subset() {
    for tag in ["en", "de", "fr", "es", "it", "ja", "zh", "ru", "pl"] {
        assert!(data_locales().iter().any(|d| d == tag), "{tag} missing from {:?}", data_locales());
    }
    assert!(l("de-AT").is_supported());
    assert!(!l("sw").is_supported());
}

#[test]
fn decimal() {
    let d = NumberOptions::default();
    assert_eq!(num("de-DE", 1234.5, &d), "1.234,5");
    assert_eq!(num("en-US", 1234.5, &d), "1,234.5");
    assert_eq!(num("fr-FR", 1234.5, &d), "1\u{202f}234,5");
    assert_eq!(num("en", 1.23456, &d), "1.235");
    assert_eq!(num("en", -0.5, &d), "-0.5");
    assert_eq!(num("en", 2.5, &opts(|o| o.maximum_fraction_digits = Some(0))), "3");
    assert_eq!(num("en", 1.0, &NumberOptions::fraction_digits(2)), "1.00");
    assert_eq!(num("en", 1234567.0, &opts(|o| o.use_grouping = Some(Grouping::Off))), "1234567");
    assert_eq!(num("es", 1234.0, &d), "1234", "es groups four-digit numbers only with min2");
    assert_eq!(num("en", 5.0, &opts(|o| o.sign_display = SignDisplay::Always)), "+5");
    assert_eq!(num("en", 0.0, &opts(|o| o.sign_display = SignDisplay::ExceptZero)), "0");
    assert_eq!(num("en", 7.0, &opts(|o| o.minimum_integer_digits = 3)), "007");
    assert_eq!(num("en", 123456.0, &opts(|o| o.maximum_significant_digits = Some(2))), "120,000");
    assert_eq!(num("en", f64::NAN, &d), "NaN");
    assert_eq!(num("en", f64::NEG_INFINITY, &d), "-∞");
}

#[test]
fn percent_currency_compact() {
    let pct = opts(|o| o.style = NumberStyle::Percent);
    assert_eq!(num("en", 0.256, &pct), "26%");
    assert_eq!(num("de", 0.256, &pct), "26\u{a0}%");
    let eur = opts(|o| {
        o.style = NumberStyle::Currency;
        o.currency = Some("EUR".into());
    });
    assert_eq!(num("de-DE", 1234.5, &eur), "1.234,50\u{a0}€");
    let usd = opts(|o| {
        o.style = NumberStyle::Currency;
        o.currency = Some("USD".into());
    });
    assert_eq!(num("en-US", 1234.5, &usd), "$1,234.50");
    assert_eq!(num("en-US", -5.0, &usd), "-$5.00");
    let jpy = opts(|o| {
        o.style = NumberStyle::Currency;
        o.currency = Some("JPY".into());
    });
    assert_eq!(num("ja", 1234.0, &jpy), "￥1,234");
    let compact = opts(|o| o.notation = Notation::CompactShort);
    assert_eq!(num("en", 1234.0, &compact), "1.2K");
    assert_eq!(num("en", 1_500_000.0, &compact), "1.5M");
    assert_eq!(num("de", 1_500_000.0, &compact), "1,5\u{a0}Mio.");
    assert_eq!(num("en", 1234.0, &opts(|o| o.notation = Notation::CompactLong)), "1.2 thousand");
}

#[test]
fn number_parts() {
    let ps = format_number_to_parts(&l("de-DE"), -1234.5, &NumberOptions::default()).unwrap();
    let kinds: Vec<(&str, &str)> = ps.iter().map(|p| (p.kind.as_str(), p.value.as_str())).collect();
    assert_eq!(kinds, [("minusSign", "-"), ("integer", "1"), ("group", "."), ("integer", "234"), ("decimal", ","), ("fraction", "5")]);
    let ps = format_number_to_parts(&l("en"), 0.5, &opts(|o| o.style = NumberStyle::Percent)).unwrap();
    assert_eq!(ps.last().unwrap().kind, "percentSign");
}

#[test]
fn parse() {
    let de = l("de-DE");
    assert_eq!(parse_number(&de, "1,5"), Some(1.5));
    assert_eq!(parse_number(&de, "1.234,5"), Some(1234.5));
    assert_eq!(parse_number(&de, "-0,25"), Some(-0.25));
    assert_eq!(parse_number(&de, "1.5"), Some(1.5), "a lone dot not grouping three digits is a decimal point");
    assert_eq!(parse_number(&de, "1.500"), Some(1500.0));
    assert_eq!(parse_number(&de, " 42 "), Some(42.0));
    assert_eq!(parse_number(&de, "abc"), None);
    assert_eq!(parse_number(&de, ""), None);
    let en = l("en-US");
    assert_eq!(parse_number(&en, "1,234.5"), Some(1234.5));
    assert_eq!(parse_number(&en, "1.5"), Some(1.5));
    assert_eq!(parse_number(&en, "1e3"), Some(1000.0));
    let fr = l("fr");
    assert_eq!(parse_number(&fr, "1 234,5"), Some(1234.5));
    assert_eq!(parse_number(&fr, "1\u{202f}234,5"), Some(1234.5));
    assert_eq!(parse_number(&fr, "\u{2212}3"), Some(-3.0));
    // round trip through the formatter
    for tag in ["en", "de", "fr", "es", "it", "ja", "zh", "ru", "pl"] {
        let loc = l(tag);
        let s = format_number(&loc, -1234567.25, &NumberOptions::default()).unwrap();
        assert_eq!(parse_number(&loc, &s), Some(-1234567.25), "{tag}: {s}");
    }
}

#[test]
fn plurals() {
    let c = PluralOptions::cardinal();
    let cat = |tag: &str, n: f64| plural_category(&l(tag), n, &c).unwrap();
    assert_eq!(cat("en", 1.0), "one");
    assert_eq!(cat("en", 2.0), "other");
    assert_eq!(cat("pl", 1.0), "one");
    assert_eq!(cat("pl", 3.0), "few");
    assert_eq!(cat("pl", 5.0), "many");
    assert_eq!(cat("pl", 22.0), "few");
    assert_eq!(cat("pl", 1.5), "other");
    assert_eq!(cat("ru", 1.0), "one");
    assert_eq!(cat("ru", 21.0), "one");
    assert_eq!(cat("ru", 3.0), "few");
    assert_eq!(cat("ru", 11.0), "many");
    assert_eq!(cat("ja", 1.0), "other");
    let o = PluralOptions::ordinal();
    let ord = |n: f64| plural_category(&l("en"), n, &o).unwrap();
    assert_eq!([ord(1.0), ord(2.0), ord(3.0), ord(4.0), ord(11.0), ord(21.0)], ["one", "two", "few", "other", "other", "one"]);
    let mut one_dp = PluralOptions::cardinal();
    one_dp.minimum_fraction_digits = Some(1);
    assert_eq!(plural_category(&l("en"), 1.0, &one_dp).unwrap(), "other");
    assert_eq!(plural_categories(&l("pl"), false).unwrap(), ["one", "few", "many", "other"]);
    assert_eq!(plural_categories(&l("en"), true).unwrap(), ["one", "two", "few", "other"]);
}

#[test]
fn collation() {
    let d = CollatorOptions::default();
    let de = l("de");
    let mut words = vec!["Zebra", "Äpfel", "Apfel", "zahl", "äffin", "Bär"];
    words.sort_by(|a, b| compare(&de, a, b, &d).unwrap());
    assert_eq!(words, ["äffin", "Apfel", "Äpfel", "Bär", "zahl", "Zebra"]);
    let mut base = CollatorOptions::default();
    base.sensitivity = Sensitivity::Base;
    assert_eq!(compare(&de, "a", "Ä", &base).unwrap(), Ordering::Equal);
    let mut accent = CollatorOptions::default();
    accent.sensitivity = Sensitivity::Accent;
    assert_eq!(compare(&de, "a", "A", &accent).unwrap(), Ordering::Equal);
    assert_ne!(compare(&de, "a", "ä", &accent).unwrap(), Ordering::Equal);
    let mut numeric = CollatorOptions::default();
    numeric.numeric = true;
    assert_eq!(compare(&l("en"), "item 10", "item 9", &numeric).unwrap(), Ordering::Greater);
    assert_eq!(compare(&l("en"), "item 10", "item 9", &d).unwrap(), Ordering::Less);
    // Swedish-style tailoring is not in the subset; Polish is: ł sorts after l
    assert_eq!(compare(&l("pl"), "łódź", "lato", &d).unwrap(), Ordering::Greater);
    assert_eq!(compare(&l("pl"), "łódź", "mama", &d).unwrap(), Ordering::Less);
}

#[test]
fn dates() {
    // 2024-03-05T14:07:09.123Z
    let t = 1_709_647_629_123.0;
    let style = |ds: Option<DateStyle>, ts: Option<DateStyle>| {
        let mut o = DateOptions::default();
        o.date_style = ds;
        o.time_style = ts;
        o
    };
    let fmt = |tag: &str, o: &DateOptions| format_date(&l(tag), t, Zone::Utc, o).unwrap();
    assert_eq!(fmt("en-US", &style(Some(DateStyle::Long), None)), "March 5, 2024");
    assert_eq!(fmt("de-DE", &style(Some(DateStyle::Long), None)), "5. März 2024");
    assert_eq!(fmt("en-US", &style(Some(DateStyle::Short), None)), "3/5/24");
    assert_eq!(fmt("de-DE", &style(Some(DateStyle::Medium), None)), "05.03.2024");
    assert_eq!(fmt("fr", &style(Some(DateStyle::Full), None)), "mardi 5 mars 2024");
    assert_eq!(fmt("ja", &style(Some(DateStyle::Long), None)), "2024年3月5日");
    assert_eq!(fmt("en-US", &style(None, Some(DateStyle::Short))), "2:07\u{202f}PM");
    assert_eq!(fmt("de-DE", &style(None, Some(DateStyle::Medium))), "14:07:09");
    assert_eq!(fmt("en-US", &DateOptions::default()), "3/5/2024");
    let mut comp = DateOptions::default();
    comp.weekday = Some(TextStyle::Long);
    comp.month = Some(MonthStyle::Long);
    comp.day = Some(NumericStyle::Numeric);
    assert_eq!(fmt("en-US", &comp), "Tuesday, March 5");
    let mut hm = DateOptions::default();
    hm.hour = Some(NumericStyle::Numeric);
    hm.minute = Some(NumericStyle::TwoDigit);
    hm.hour_cycle = Some(HourCycle::H23);
    assert_eq!(fmt("en-US", &hm), "14:07");
    // the local zone as an offset: 14:07Z is 16:07 at +02:00
    let mut tz = hm.clone();
    tz.time_zone_name = Some(TimeZoneNameStyle::ShortOffset);
    assert_eq!(format_date(&l("en-US"), t, Zone::Offset(7200), &tz).unwrap(), "16:07 GMT+2");
    let ps = format_date_to_parts(&l("de-DE"), t, Zone::Utc, &style(Some(DateStyle::Medium), None)).unwrap();
    let kinds: Vec<&str> = ps.iter().map(|p| p.kind.as_str()).collect();
    assert_eq!(kinds, ["day", "literal", "month", "literal", "year"]);
    assert!(format_date(&l("en"), f64::NAN, Zone::Utc, &DateOptions::default()).is_err());
    let mut bad = style(Some(DateStyle::Long), None);
    bad.year = Some(NumericStyle::Numeric);
    assert!(format_date(&l("en"), t, Zone::Utc, &bad).is_err());
}

#[test]
fn locales() {
    assert_eq!(canonicalize("EN-us").unwrap(), "en-US");
    assert!(canonicalize("not a tag").is_err());
    assert_eq!(resolve(&["sw", "de-CH"]).unwrap().tag(), "de-CH");
    assert_eq!(Locale::parse_lenient("de_DE.UTF-8").unwrap().tag(), "de-DE");
    let g = current_generation();
    set_current("pl-PL").unwrap();
    assert_eq!(current().tag(), "pl-PL");
    assert_eq!(current_generation(), g + 1);
    assert_eq!(resolve(&["sw"]).unwrap().tag(), "pl-PL", "unsupported requests fall back to the current locale");
}
