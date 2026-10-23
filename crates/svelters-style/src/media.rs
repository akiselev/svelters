//! `@media` query lists and their evaluation against a [`MediaEnv`] (CSS.md §2.4 item 6).
//!
//! Media types `all`, `screen` (true) and `print` (false); the features `width`, `height` (with
//! `min-`/`max-` and the range syntax), `orientation`, `aspect-ratio`, `prefers-color-scheme`,
//! `prefers-reduced-motion` and `prefers-contrast`; `not`, `and`, `or` and `only`. An unknown
//! feature or type evaluates false, as in browsers.

use cssparser::{ParseError, Parser, Token};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Contrast {
    #[default]
    NoPreference,
    More,
    Less,
    Custom,
}

/// What media queries see: the toplevel's size and the platform's preferences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaEnv {
    /// The toplevel's size in px.
    pub width: f32,
    pub height: f32,
    pub color_scheme: ColorScheme,
    pub reduced_motion: bool,
    pub contrast: Contrast,
    /// `1em` in a media query (CSS: the initial font size).
    pub em: f32,
}

impl Default for MediaEnv {
    fn default() -> Self {
        MediaEnv { width: 1024.0, height: 768.0, color_scheme: ColorScheme::Light, reduced_motion: false, contrast: Contrast::NoPreference, em: 16.0 }
    }
}

/// A comma-separated media query list: true when any query is.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaList {
    pub(crate) queries: Vec<Query>,
    /// The source text, for diagnostics and dumps.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Query {
    negated: bool,
    /// `None` = `all`.
    media_type: Option<String>,
    condition: Option<Cond>,
}

#[derive(Clone, Debug, PartialEq)]
enum Cond {
    Not(Box<Cond>),
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Feature(Feature),
    /// An unknown feature or a general-enclosed term: false.
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
enum Feature {
    /// `(width)`: true when the value is non-zero / not `none`.
    Boolean(String),
    /// `(name: value)` for a discrete feature.
    Discrete(String, String),
    /// `name op value` for a range feature, normalised to `feature op value`.
    Range(String, Op, f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

impl Op {
    fn flip(self) -> Op {
        match self {
            Op::Lt => Op::Gt,
            Op::Le => Op::Ge,
            Op::Eq => Op::Eq,
            Op::Ge => Op::Le,
            Op::Gt => Op::Lt,
        }
    }
    fn test(self, a: f32, b: f32) -> bool {
        match self {
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Eq => a == b,
            Op::Ge => a >= b,
            Op::Gt => a > b,
        }
    }
}

type PResult<'i, T> = Result<T, cssparser::ParseError<String>>;

impl MediaList {
    /// Parses the prelude of `@media`. An empty list is `all`.
    pub fn parse(text: &str) -> Result<MediaList, String> {
        let mut input = Parser::new(text);
        let mut queries = Vec::new();
        if !input.is_exhausted() {
            queries = input
                .parse_comma_separated(|i| {
                    // A query that fails to parse is `not all` (Media Queries 4 §3.2).
                    let state = i.state();
                    match parse_query(i) {
                        Ok(q) => Ok(q),
                        Err(_) => {
                            i.reset(&state);
                            while i.next().is_ok() {}
                            Ok::<_, cssparser::ParseError<String>>(Query { negated: true, media_type: None, condition: None })
                        }
                    }
                })
                .map_err(|e| format!("{:?}", e.kind))?;
        }
        Ok(MediaList { queries, text: text.trim().to_string() })
    }

    pub fn matches(&self, env: &MediaEnv) -> bool {
        self.queries.is_empty() || self.queries.iter().any(|q| q.matches(env))
    }

    /// Whether it reads the viewport size (so a resize can change it).
    pub fn depends_on_size(&self) -> bool {
        self.queries.iter().any(|q| q.condition.as_ref().is_some_and(Cond::reads_size))
    }
}

impl Query {
    fn matches(&self, env: &MediaEnv) -> bool {
        let type_ok = match self.media_type.as_deref() {
            None | Some("all") | Some("screen") => true,
            Some(_) => false,
        };
        let r = type_ok && self.condition.as_ref().is_none_or(|c| c.eval(env));
        r != self.negated
    }
}

impl Cond {
    fn eval(&self, env: &MediaEnv) -> bool {
        match self {
            Cond::Not(c) => !c.eval(env),
            Cond::And(cs) => cs.iter().all(|c| c.eval(env)),
            Cond::Or(cs) => cs.iter().any(|c| c.eval(env)),
            Cond::Feature(f) => f.eval(env),
            Cond::Unknown => false,
        }
    }

    fn reads_size(&self) -> bool {
        match self {
            Cond::Not(c) => c.reads_size(),
            Cond::And(cs) | Cond::Or(cs) => cs.iter().any(Cond::reads_size),
            Cond::Feature(Feature::Range(n, ..)) | Cond::Feature(Feature::Boolean(n)) | Cond::Feature(Feature::Discrete(n, _)) => {
                matches!(n.as_str(), "width" | "height" | "aspect-ratio" | "orientation")
            }
            Cond::Unknown => false,
        }
    }
}

impl Feature {
    fn eval(&self, env: &MediaEnv) -> bool {
        match self {
            Feature::Boolean(n) => match n.as_str() {
                "width" => env.width > 0.0,
                "height" => env.height > 0.0,
                "prefers-reduced-motion" => env.reduced_motion,
                "prefers-contrast" => env.contrast != Contrast::NoPreference,
                "prefers-color-scheme" | "orientation" | "aspect-ratio" => true,
                _ => false,
            },
            Feature::Discrete(n, v) => match (n.as_str(), v.as_str()) {
                ("prefers-color-scheme", "light") => env.color_scheme == ColorScheme::Light,
                ("prefers-color-scheme", "dark") => env.color_scheme == ColorScheme::Dark,
                ("prefers-reduced-motion", "reduce") => env.reduced_motion,
                ("prefers-reduced-motion", "no-preference") => !env.reduced_motion,
                ("prefers-contrast", "no-preference") => env.contrast == Contrast::NoPreference,
                ("prefers-contrast", "more") => env.contrast == Contrast::More,
                ("prefers-contrast", "less") => env.contrast == Contrast::Less,
                ("prefers-contrast", "custom") => env.contrast == Contrast::Custom,
                ("orientation", "portrait") => env.height >= env.width,
                ("orientation", "landscape") => env.width > env.height,
                _ => false,
            },
            Feature::Range(n, op, v) => {
                let actual = match n.as_str() {
                    "width" => env.width,
                    "height" => env.height,
                    "aspect-ratio" if env.height > 0.0 => env.width / env.height,
                    _ => return false,
                };
                op.test(actual, *v)
            }
        }
    }
}

fn parse_query<'i>(input: &mut Parser<'i>) -> PResult<'i, Query> {
    // `(cond)` or `not (cond)` without a type
    if let Ok(c) = input.try_parse(parse_condition) {
        input.expect_exhausted()?;
        return Ok(Query { negated: false, media_type: None, condition: Some(c) });
    }
    let mut negated = false;
    let first = input.expect_ident_cloned()?.to_ascii_lowercase();
    let media_type = match first.as_str() {
        "not" => {
            negated = true;
            input.expect_ident_cloned()?.to_ascii_lowercase()
        }
        "only" => input.expect_ident_cloned()?.to_ascii_lowercase(),
        _ => first,
    };
    let condition = if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        Some(parse_and_chain(input)?)
    } else {
        None
    };
    input.expect_exhausted()?;
    Ok(Query { negated, media_type: Some(media_type), condition })
}

/// `<media-condition>`: `not X`, or `X [and X]*`, or `X [or X]*`.
fn parse_condition<'i>(input: &mut Parser<'i>) -> PResult<'i, Cond> {
    if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
        return Ok(Cond::Not(Box::new(parse_in_parens(input)?)));
    }
    let first = parse_in_parens(input)?;
    let mut and = vec![];
    let mut or = vec![];
    loop {
        if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            and.push(parse_in_parens(input)?);
        } else if input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
            or.push(parse_in_parens(input)?);
        } else {
            break;
        }
    }
    match (and.is_empty(), or.is_empty()) {
        (true, true) => Ok(first),
        (false, true) => Ok(Cond::And(std::iter::once(first).chain(and).collect())),
        (true, false) => Ok(Cond::Or(std::iter::once(first).chain(or).collect())),
        (false, false) => Err(ParseError::custom("`and` and `or` mixed without parentheses".to_string())),
    }
}

/// After `<type> and`: `X [and X]*` (no `or`).
fn parse_and_chain<'i>(input: &mut Parser<'i>) -> PResult<'i, Cond> {
    let mut all = vec![parse_not_or_in_parens(input)?];
    while input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        all.push(parse_not_or_in_parens(input)?);
    }
    Ok(if all.len() == 1 { all.pop().unwrap() } else { Cond::And(all) })
}

fn parse_not_or_in_parens<'i>(input: &mut Parser<'i>) -> PResult<'i, Cond> {
    if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
        return Ok(Cond::Not(Box::new(parse_in_parens(input)?)));
    }
    parse_in_parens(input)
}

fn parse_in_parens<'i>(input: &mut Parser<'i>) -> PResult<'i, Cond> {
    input.expect_parenthesis_block()?;
    input.parse_nested_block(|i| {
        if let Ok(c) = i.try_parse(parse_condition) {
            return Ok(c);
        }
        let state = i.state();
        match parse_feature(i) {
            Ok(f) => Ok(f),
            Err(_) => {
                // general-enclosed: valid syntax, evaluates false
                i.reset(&state);
                while i.next().is_ok() {}
                Ok(Cond::Unknown)
            }
        }
    })
}

const RANGE: &[&str] = &["width", "height", "aspect-ratio"];
const DISCRETE: &[&str] = &["prefers-color-scheme", "prefers-reduced-motion", "prefers-contrast", "orientation"];

/// U0 (UTILITY-CSS §3.4): the media features this engine evaluates, for the semantic spec
/// (`style.media_features`), the utility variants and the docs: `(name, "range" | "discrete", the
/// discrete values)`. The media tests keep it equal to what the evaluator knows.
pub const MEDIA_FEATURES: &[(&str, &str, &[&str])] = &[
    ("width", "range", &[]),
    ("height", "range", &[]),
    ("aspect-ratio", "range", &[]),
    ("orientation", "discrete", &["portrait", "landscape"]),
    ("prefers-color-scheme", "discrete", &["light", "dark"]),
    ("prefers-contrast", "discrete", &["no-preference", "more", "less", "custom"]),
    ("prefers-reduced-motion", "discrete", &["no-preference", "reduce"]),
];

fn parse_feature<'i>(input: &mut Parser<'i>) -> PResult<'i, Cond> {
    // `value op name [op value]`
    if let Ok(v) = input.try_parse(feature_value) {
        let op1 = parse_op(input)?;
        let name = input.expect_ident_cloned()?.to_ascii_lowercase();
        if !RANGE.contains(&name.as_str()) {
            return Ok(Cond::Unknown);
        }
        let first = Cond::Feature(Feature::Range(name.clone(), op1.flip(), v));
        if input.is_exhausted() {
            return Ok(first);
        }
        let op2 = parse_op(input)?;
        let v2 = feature_value(input)?;
        input.expect_exhausted()?;
        return Ok(Cond::And(vec![first, Cond::Feature(Feature::Range(name, op2, v2))]));
    }
    let name = input.expect_ident_cloned()?.to_ascii_lowercase();
    if input.is_exhausted() {
        return Ok(if RANGE.contains(&name.as_str()) || DISCRETE.contains(&name.as_str()) {
            Cond::Feature(Feature::Boolean(name))
        } else {
            Cond::Unknown
        });
    }
    if input.try_parse(|i| i.expect_colon()).is_ok() {
        if let Some(base) = name.strip_prefix("min-").or_else(|| name.strip_prefix("max-")) {
            if !RANGE.contains(&base) {
                return Ok(Cond::Unknown);
            }
            let v = feature_value(input)?;
            input.expect_exhausted()?;
            let op = if name.starts_with("min-") { Op::Ge } else { Op::Le };
            return Ok(Cond::Feature(Feature::Range(base.to_string(), op, v)));
        }
        if RANGE.contains(&name.as_str()) {
            let v = feature_value(input)?;
            input.expect_exhausted()?;
            return Ok(Cond::Feature(Feature::Range(name, Op::Eq, v)));
        }
        let v = input.expect_ident_cloned()?.to_ascii_lowercase();
        input.expect_exhausted()?;
        return Ok(if DISCRETE.contains(&name.as_str()) { Cond::Feature(Feature::Discrete(name, v)) } else { Cond::Unknown });
    }
    let op = parse_op(input)?;
    let v = feature_value(input)?;
    input.expect_exhausted()?;
    Ok(if RANGE.contains(&name.as_str()) { Cond::Feature(Feature::Range(name, op, v)) } else { Cond::Unknown })
}

fn parse_op<'i>(input: &mut Parser<'i>) -> PResult<'i, Op> {
    let c = match input.next()? {
        Token::Delim(c) => *c,
        _ => return Err(ParseError::unexpected_token()),
    };
    let eq = input.try_parse(|i| i.expect_delim('=')).is_ok();
    Ok(match (c, eq) {
        ('<', false) => Op::Lt,
        ('<', true) => Op::Le,
        ('>', false) => Op::Gt,
        ('>', true) => Op::Ge,
        ('=', false) => Op::Eq,
        _ => return Err(ParseError::custom(format!("bad comparison `{c}`"))),
    })
}

/// A length in px (`px`, `em`, `rem`, unitless 0), or a ratio `a / b`, or a number.
fn feature_value<'i>(input: &mut Parser<'i>) -> PResult<'i, f32> {
    let em = 16.0;
    let v = match input.next()?.clone() {
        Token::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
            "px" => value,
            "em" | "rem" => value * em,
            _ => return Err(ParseError::custom(format!("unit `{unit}` in a media query"))),
        },
        Token::Number { value, .. } => {
            if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                let d = input.expect_number()?;
                if d == 0.0 {
                    return Err(ParseError::custom("ratio with a zero denominator".to_string()));
                }
                value / d
            } else {
                value
            }
        }
        _ => return Err(ParseError::unexpected_token()),
    };
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(q: &str, env: &MediaEnv) -> bool {
        MediaList::parse(q).unwrap_or_else(|e| panic!("{q}: {e}")).matches(env)
    }

    /// U0: `MEDIA_FEATURES` lists exactly the features the parser keeps, and every discrete value
    /// it lists is one some environment matches (an unknown value would never match).
    #[test]
    fn media_features_table_matches_the_evaluator() {
        let range: Vec<&str> = MEDIA_FEATURES.iter().filter(|f| f.1 == "range").map(|f| f.0).collect();
        let discrete: Vec<&str> = MEDIA_FEATURES.iter().filter(|f| f.1 == "discrete").map(|f| f.0).collect();
        assert_eq!(range.len() + discrete.len(), MEDIA_FEATURES.len());
        fn sorted<'a>(v: &[&'a str]) -> Vec<&'a str> {
            let mut v = v.to_vec();
            v.sort_unstable();
            v
        }
        assert_eq!(sorted(&range), sorted(RANGE));
        assert_eq!(sorted(&discrete), sorted(DISCRETE));
        let envs: Vec<MediaEnv> = [ColorScheme::Light, ColorScheme::Dark]
            .into_iter()
            .flat_map(|color_scheme| {
                [Contrast::NoPreference, Contrast::More, Contrast::Less, Contrast::Custom].into_iter().flat_map(move |contrast| {
                    [false, true].into_iter().flat_map(move |reduced_motion| {
                        [(800.0, 600.0), (600.0, 800.0)].map(|(width, height)| MediaEnv { width, height, color_scheme, contrast, reduced_motion, ..Default::default() })
                    })
                })
            })
            .collect();
        for (name, _, values) in MEDIA_FEATURES {
            for v in *values {
                let q = format!("({name}: {v})");
                assert!(envs.iter().any(|e| m(&q, e)), "{q} never matches");
            }
        }
    }

    #[test]
    fn evaluates() {
        let env = MediaEnv { width: 800.0, height: 600.0, color_scheme: ColorScheme::Dark, ..Default::default() };
        assert!(m("", &env));
        assert!(m("screen", &env));
        assert!(!m("print", &env));
        assert!(m("not print", &env));
        assert!(m("(prefers-color-scheme: dark)", &env));
        assert!(!m("(prefers-color-scheme: light)", &env));
        assert!(m("(min-width: 600px)", &env));
        assert!(!m("(max-width: 600px)", &env));
        assert!(m("(width >= 800px)", &env));
        assert!(!m("(width > 800px)", &env));
        assert!(m("(400px <= width <= 50em)", &env));
        assert!(!m("(400px <= width <= 700px)", &env));
        assert!(m("screen and (min-width: 30em) and (orientation: landscape)", &env));
        assert!(m("only screen and (min-width: 1px)", &env));
        assert!(m("(max-width: 100px) or (prefers-color-scheme: dark)", &env));
        assert!(m("not (prefers-reduced-motion: reduce)", &env));
        assert!(m("(min-aspect-ratio: 4/3)", &env));
        assert!(!m("(unknown-feature: 1)", &env));
        assert!(m("(unknown-feature: 1), screen", &env));
        assert!(!m("garbage !!", &env));
        assert!(MediaList::parse("(min-width: 1px)").unwrap().depends_on_size());
        assert!(!MediaList::parse("(prefers-color-scheme: dark)").unwrap().depends_on_size());
    }
}
