//! Fonts bundled for deterministic tests (`crates/presto-text/fonts/`, SIL Open Font License 1.1,
//! see `fonts/OFL.txt` and `fonts/README.md`): subsets of Noto Sans (Latin-1 and punctuation,
//! hinted TrueType), Noto Sans Hebrew (hinted) and Noto Color Emoji (COLRv1, three emoji).
//! [`crate::FontConfig::testing`] loads them with system fonts off.

pub const SANS_REGULAR: &[u8] = include_bytes!("../fonts/NotoSans-Regular.subset.ttf");
pub const SANS_BOLD: &[u8] = include_bytes!("../fonts/NotoSans-Bold.subset.ttf");
pub const HEBREW: &[u8] = include_bytes!("../fonts/NotoSansHebrew-Regular.subset.ttf");
pub const EMOJI_COLRV1: &[u8] = include_bytes!("../fonts/NotoColorEmoji-COLRv1.subset.ttf");

pub const SANS_FAMILY: &str = "Noto Sans";
pub const HEBREW_FAMILY: &str = "Noto Sans Hebrew";
pub const EMOJI_FAMILY: &str = "Noto Color Emoji";

pub const ALL: &[&[u8]] = &[SANS_REGULAR, SANS_BOLD, HEBREW, EMOJI_COLRV1];
