//! Thread-local cache for parsed `syn::File` ASTs.
//!
//! `RuleRegistry::run_all` invokes every registered [`crate::rules::Rule`] against the
//! *same* source text for a given file (once for the original source and once for the
//! macro-expanded source). Historically each rule's `check` method called
//! `syn::parse_str::<File>(source)` independently, so a single source string was
//! lexed and parsed roughly as many times as there are rules. Lexing/parsing is the
//! expensive part of that work; this module lets identical-source parses be served
//! from a small in-process cache instead of being redone by every rule.
//!
//! The cache is intentionally simple: a plain unbounded `HashMap` keyed by a fast
//! hash of the source string, holding an `Rc<syn::File>` so cloning a cache hit is
//! just a refcount bump (rules still `.clone()` the `syn::File` out of the `Rc`
//! before mutating/consuming it, which costs an AST clone but not a re-parse). No
//! eviction/LRU logic is needed because a single `sanctifier` scan process parses a
//! bounded, per-run set of source files (at most a few times each), so the cache
//! never grows large enough to matter, and the cache does not outlive the process.
//! Parse failures are deliberately not cached, since a caller could reasonably pass
//! different content sharing the same source string only by coincidence, and caching
//! `None` would save nothing (parsing invalid, typically tiny/malformed, input to
//! find out it fails is not the expensive case this module targets).

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

thread_local! {
    static PARSE_CACHE: RefCell<HashMap<u64, Rc<syn::File>>> = RefCell::new(HashMap::new());
}

/// Hashes just this one source string (not the whole workspace) with the default
/// `SipHash`-based hasher. This is cheap and collision-resistant enough for a
/// same-process cache key; it is not used for anything security-sensitive.
fn hash_source(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Returns the parsed `syn::File` for `source`, serving it from the thread-local
/// cache when the same source text has already been parsed once.
///
/// On a cache miss, this parses `source` with `syn::parse_str::<syn::File>`. A
/// successful parse is cached (as an `Rc`, so subsequent hits are cheap to clone
/// out) and returned; a parse failure is *not* cached and yields `None`.
pub fn parse_cached(source: &str) -> Option<Rc<syn::File>> {
    let key = hash_source(source);

    if let Some(cached) = PARSE_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(cached);
    }

    let file = syn::parse_str::<syn::File>(source).ok()?;
    let file = Rc::new(file);
    PARSE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, Rc::clone(&file));
    });
    Some(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_same_source_twice_returns_cached_data() {
        let source = "fn foo() -> u32 { 1 + 2 }";

        let first = parse_cached(source).expect("valid source should parse");
        let second = parse_cached(source).expect("valid source should parse");

        // Same cache entry: both parses agree on content, and the second call
        // was served from the cache (same underlying allocation via Rc).
        assert_eq!(*first, *second);
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn invalid_source_returns_none_without_panicking() {
        let source = "fn this is not valid rust {{{";

        assert!(parse_cached(source).is_none());
        // Calling it again should still not panic or cache anything usable.
        assert!(parse_cached(source).is_none());
    }
}
