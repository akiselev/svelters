//! W-C1 (presto `compiler-and-js.md` §3.1): the spec hash of each backend in the process. A backend
//! whose semantic spec carries stable ids registers the hash of those ids here; `host.specHash(backend)`
//! answers it to the runtime, which compares it with the hash a module was compiled against
//! (`@svelters/native-runtime` `ids`) instead of checking each id through the resolvers.

use std::sync::Mutex;

static HASHES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Registers (or replaces) the spec hash of `backend` (`egui`, ...).
pub fn register_spec_hash(backend: &str, hash: String) {
    let mut v = HASHES.lock().unwrap_or_else(|e| e.into_inner());
    match v.iter_mut().find(|(b, _)| b == backend) {
        Some(e) => e.1 = hash,
        None => v.push((backend.to_string(), hash)),
    }
}

/// The spec hash `backend` registered, if any.
pub fn spec_hash(backend: &str) -> Option<String> {
    HASHES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(b, _)| b == backend).map(|(_, h)| h.clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes_register_by_backend() {
        super::register_spec_hash("test-backend", "a".into());
        super::register_spec_hash("test-backend", "b".into());
        assert_eq!(super::spec_hash("test-backend").as_deref(), Some("b"));
        assert_eq!(super::spec_hash("test-other"), None);
    }
}
