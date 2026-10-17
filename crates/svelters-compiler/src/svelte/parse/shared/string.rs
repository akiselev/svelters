//! `list` of `compiler/utils/string.js` (Svelte 5.57.2 @ f3de3f2).

pub fn list(strings: &[&str], conjunction: &str) -> String {
    if strings.len() == 1 {
        return strings[0].to_string();
    }
    if strings.len() == 2 {
        return format!("{} {conjunction} {}", strings[0], strings[1]);
    }
    format!("{} {conjunction} {}", strings[..strings.len() - 1].join(", "), strings[strings.len() - 1])
}
