//! Conservative, offline macro-expansion pass so detectors can see through
//! locally-defined `macro_rules!` wrappers.
//!
//! Soroban code leans heavily on macros, and logic hidden behind a local macro
//! (a wrapped `unwrap()`, a raw `/`, a missing auth check, …) is invisible to
//! AST-level detectors, producing false negatives. This module performs a
//! deliberately bounded, single-level expansion of *simple* local macros and
//! emits the expanded call sites as ordinary functions inside a synthetic
//! module, which the existing detectors then analyse like any other code.
//!
//! It is intentionally conservative: anything it cannot expand with confidence
//! (repetitions `$(...)*`, multi-rule macros, non-trivial matchers, results
//! that don't parse) is skipped, so the pass never changes behaviour on code it
//! doesn't understand and never introduces false *positives* from bad guesses.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::HashMap;
use std::str::FromStr;

/// A single-rule `macro_rules!` we know how to expand: an ordered list of
/// metavariable names and the transcriber token stream.
struct SimpleMacro {
    params: Vec<String>,
    body: TokenStream,
}

/// Expand simple local macros in `source` and return a synthetic Rust module
/// (as a string) containing one function per expanded call site. Returns
/// `None` when there is nothing to expand or the result would not parse.
pub fn expand_local_macros(source: &str) -> Option<String> {
    let stream = TokenStream::from_str(source).ok()?;

    let mut macros: HashMap<String, SimpleMacro> = HashMap::new();
    collect_macros(stream.clone(), &mut macros);
    if macros.is_empty() {
        return None;
    }

    let mut expansions: Vec<TokenStream> = Vec::new();
    collect_expansions(stream, &macros, &mut expansions);
    if expansions.is_empty() {
        return None;
    }

    // Emit the expanded call sites as methods on a synthetic impl. Using an
    // `impl` (rather than free functions) means both free-function and
    // impl-scanning detectors see the expanded bodies, while keeping them
    // private and attribute-free so entrypoint heuristics (e.g. auth gaps)
    // don't fire spuriously on generated code.
    let mut out =
        String::from("struct __SanctifierMacroExpanded;\nimpl __SanctifierMacroExpanded {\n");
    for (i, ts) in expansions.iter().enumerate() {
        out.push_str(&format!(
            "    fn __expanded_{i}() {{\n        {ts}\n    }}\n"
        ));
    }
    out.push_str("}\n");

    // Only hand back an expansion that is itself valid Rust; otherwise a
    // detector re-parsing it would just get an empty result anyway.
    if syn::parse_str::<syn::File>(&out).is_ok() {
        Some(out)
    } else {
        None
    }
}

/// Recursively find `macro_rules! name { (matcher) => { body } ; ... }`
/// definitions we can handle, keyed by name.
fn collect_macros(stream: TokenStream, out: &mut HashMap<String, SimpleMacro>) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        // Pattern: `macro_rules` `!` Ident(name) Group{ rules }
        if let TokenTree::Ident(kw) = &tokens[i] {
            if kw == "macro_rules"
                && matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
            {
                if let (Some(TokenTree::Ident(name)), Some(TokenTree::Group(g))) =
                    (tokens.get(i + 2), tokens.get(i + 3))
                {
                    if let Some(m) = parse_simple_macro(g.stream()) {
                        out.insert(name.to_string(), m);
                    }
                    i += 4;
                    continue;
                }
            }
        }
        // Recurse into any grouping (modules, impls, fns, …).
        if let TokenTree::Group(g) = &tokens[i] {
            collect_macros(g.stream(), out);
        }
        i += 1;
    }
}

/// Parse the first rule of a `macro_rules!` body when it is simple enough:
/// a flat, comma-separated list of `$name:frag` metavariables with no
/// repetitions or literal matcher tokens.
fn parse_simple_macro(rules: TokenStream) -> Option<SimpleMacro> {
    let toks: Vec<TokenTree> = rules.into_iter().collect();

    // matcher = first parenthesised group; transcriber = the next group after
    // the `=>`.
    let matcher_idx = toks.iter().position(
        |t| matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis),
    )?;
    let matcher = match &toks[matcher_idx] {
        TokenTree::Group(g) => g.stream(),
        _ => return None,
    };
    let body_idx =
        (matcher_idx + 1..toks.len()).find(|&j| matches!(&toks[j], TokenTree::Group(_)))?;
    let body = match &toks[body_idx] {
        TokenTree::Group(g) => g.stream(),
        _ => return None,
    };

    let params = parse_matcher_params(matcher)?;
    if params.is_empty() {
        return None;
    }
    Some(SimpleMacro { params, body })
}

/// Extract metavariable names from a matcher, bailing (returning `None`) on any
/// construct we don't handle: repetitions, nested groups, or literal tokens.
fn parse_matcher_params(matcher: TokenStream) -> Option<Vec<String>> {
    let toks: Vec<TokenTree> = matcher.into_iter().collect();
    let mut params = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            // `$ name : frag`
            TokenTree::Punct(p) if p.as_char() == '$' => {
                let name = match toks.get(i + 1) {
                    Some(TokenTree::Ident(id)) => id.to_string(),
                    _ => return None, // e.g. `$(` repetition
                };
                if !matches!(toks.get(i + 2), Some(TokenTree::Punct(c)) if c.as_char() == ':') {
                    return None;
                }
                if !matches!(toks.get(i + 3), Some(TokenTree::Ident(_))) {
                    return None; // fragment specifier
                }
                params.push(name);
                i += 4;
            }
            // Separator between metavariables.
            TokenTree::Punct(p) if p.as_char() == ',' => i += 1,
            // Anything else (literal matcher tokens, groups) → not simple.
            _ => return None,
        }
    }
    Some(params)
}

/// Recursively find invocations of known macros and record their expansions.
fn collect_expansions(
    stream: TokenStream,
    macros: &HashMap<String, SimpleMacro>,
    out: &mut Vec<TokenStream>,
) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        // Pattern: Ident(name) `!` Group(args)
        if let TokenTree::Ident(name) = &tokens[i] {
            if matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!') {
                if let Some(TokenTree::Group(args)) = tokens.get(i + 2) {
                    if name != "macro_rules" {
                        if let Some(m) = macros.get(&name.to_string()) {
                            let arg_streams = split_top_level_commas(args.stream());
                            if arg_streams.len() == m.params.len() {
                                let bindings: HashMap<String, TokenStream> =
                                    m.params.iter().cloned().zip(arg_streams).collect();
                                out.push(substitute(m.body.clone(), &bindings));
                            }
                        }
                        i += 3;
                        continue;
                    }
                }
            }
        }
        if let TokenTree::Group(g) = &tokens[i] {
            collect_expansions(g.stream(), macros, out);
        }
        i += 1;
    }
}

/// Split a token stream on top-level commas into argument streams.
fn split_top_level_commas(stream: TokenStream) -> Vec<TokenStream> {
    let mut args = Vec::new();
    let mut current: Vec<TokenTree> = Vec::new();
    for tt in stream {
        match &tt {
            TokenTree::Punct(p) if p.as_char() == ',' => {
                args.push(current.drain(..).collect());
            }
            _ => current.push(tt),
        }
    }
    if !current.is_empty() {
        args.push(current.into_iter().collect());
    }
    args
}

/// Replace `$name` occurrences in `body` with their bound argument tokens,
/// recursing into groups.
fn substitute(body: TokenStream, bindings: &HashMap<String, TokenStream>) -> TokenStream {
    let toks: Vec<TokenTree> = body.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            TokenTree::Punct(p) if p.as_char() == '$' => {
                if let Some(TokenTree::Ident(id)) = toks.get(i + 1) {
                    if let Some(binding) = bindings.get(&id.to_string()) {
                        out.extend(binding.clone());
                        i += 2;
                        continue;
                    }
                }
                out.push(toks[i].clone());
                i += 1;
            }
            TokenTree::Group(g) => {
                let inner = substitute(g.stream(), bindings);
                out.push(TokenTree::Group(proc_macro2::Group::new(
                    g.delimiter(),
                    inner,
                )));
                i += 1;
            }
            other => {
                out.push(other.clone());
                i += 1;
            }
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_simple_expr_macro() {
        let src = r#"
            macro_rules! divide { ($a:expr, $b:expr) => { $a / $b }; }
            fn f(x: i128, y: i128) -> i128 { divide!(x, y) }
        "#;
        let expanded = expand_local_macros(src).expect("should expand");
        assert!(expanded.contains("x / y"));
        assert!(syn::parse_str::<syn::File>(&expanded).is_ok());
    }

    #[test]
    fn no_macros_returns_none() {
        let src = "fn f(x: i128) -> i128 { x + 1 }";
        assert!(expand_local_macros(src).is_none());
    }

    #[test]
    fn skips_repetition_macros() {
        // Repetition matchers are not "simple"; nothing should expand.
        let src = r#"
            macro_rules! sum { ($($x:expr),*) => { 0 $(+ $x)* }; }
            fn f() -> i128 { sum!(1, 2, 3) }
        "#;
        assert!(expand_local_macros(src).is_none());
    }
}
