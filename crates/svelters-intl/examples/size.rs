//! IN1 size probe: links every formatter svelters-intl offers (what `Intl` in the host pulls in).
//! `cargo build --release -p svelters-intl --example size` with and without `--features probe`:
//! the difference is the binary cost of the `intl` feature (code plus the data blob).

fn main() {
    if std::env::args().len() > 99 {
        return;
    }
    #[cfg(feature = "probe")]
    {
        use svelters_intl::*;
        let l = Locale::parse("de-DE").unwrap();
        let mut out = String::new();
        let mut cur = NumberOptions::default();
        cur.style = NumberStyle::Currency;
        cur.currency = Some("EUR".into());
        let mut compact = NumberOptions::default();
        compact.notation = Notation::CompactLong;
        let mut pct = NumberOptions::default();
        pct.style = NumberStyle::Percent;
        for o in [NumberOptions::default(), cur, compact, pct] {
            out += &format_number(&l, 1234.5, &o).unwrap();
        }
        out += plural_category(&l, 2.0, &PluralOptions::cardinal()).unwrap();
        out += &format!("{:?}", compare(&l, "a", "b", &CollatorOptions::default()).unwrap());
        let mut d = DateOptions::default();
        d.date_style = Some(DateStyle::Full);
        d.time_style = Some(DateStyle::Full);
        out += &format_date(&l, 0.0, Zone::Utc, &d).unwrap();
        out += &parse_number(&l, "1,5").unwrap().to_string();
        println!("{out}");
    }
}
