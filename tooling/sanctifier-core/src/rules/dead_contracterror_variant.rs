use crate::rules::{Rule, RuleViolation, Severity};
use proc_macro2::{TokenStream, TokenTree};
use std::collections::HashSet;
use syn::visit::{self, Visit};
use syn::{ItemEnum, ItemImpl, ItemUse, Macro, Path, Type, UseTree};

/// Advisory, source-local detector: treat uncertain macro/import uses as live
/// rather than tell a contributor to delete a potentially exported ABI variant.
pub struct DeadContracterrorVariantRule;

impl DeadContracterrorVariantRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeadContracterrorVariantRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DeadContracterrorVariantRule {
    fn name(&self) -> &str {
        "dead_contracterror_variant"
    }

    fn description(&self) -> &str {
        "Flags #[contracterror] variants never referenced within a source file"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let parsed = match crate::parse_cache::parse_cached(source) {
            Some(file) => file,
            None => return Vec::new(),
        };
        let mut collector = ErrorEnumCollector::default();
        collector.visit_file(&parsed);

        let mut violations = Vec::new();
        for error_enum in collector.enums {
            let variants: HashSet<String> = error_enum
                .variants
                .iter()
                .map(|(name, _)| name.clone())
                .collect();
            let mut usage = VariantUsage {
                enum_name: &error_enum.name,
                variants: &variants,
                used: HashSet::new(),
                inside_enum_impl: false,
            };
            usage.visit_file(&parsed);
            for (variant, line) in error_enum.variants {
                if !usage.used.contains(&variant) {
                    violations.push(
                        RuleViolation::new(
                            "SANCT_DEAD_CONTRACTERROR_VARIANT",
                            Severity::Warning,
                            format!(
                                "Unused #[contracterror] variant {}::{} (no source-local reference)",
                                error_enum.name, variant
                            ),
                            format!("{}:{}", variant, line),
                        )
                        .with_suggestion(
                            "Check cross-module/public ABI uses before removing this variant; \
                             otherwise return or construct it, or remove the dead declaration"
                                .to_string(),
                        ),
                    );
                }
            }
        }
        violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ErrorEnum {
    name: String,
    variants: Vec<(String, usize)>,
}

#[derive(Default)]
struct ErrorEnumCollector {
    enums: Vec<ErrorEnum>,
}

impl<'ast> Visit<'ast> for ErrorEnumCollector {
    fn visit_item_enum(&mut self, item: &'ast ItemEnum) {
        if item.attrs.iter().any(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "contracterror")
        }) {
            self.enums.push(ErrorEnum {
                name: item.ident.to_string(),
                variants: item
                    .variants
                    .iter()
                    .map(|variant| (variant.ident.to_string(), variant.ident.span().start().line))
                    .collect(),
            });
        }
        visit::visit_item_enum(self, item);
    }
}

struct VariantUsage<'a> {
    enum_name: &'a str,
    variants: &'a HashSet<String>,
    used: HashSet<String>,
    inside_enum_impl: bool,
}

impl VariantUsage<'_> {
    fn mark_path(&mut self, path: &Path) {
        let names: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        for pair in names.windows(2) {
            if (pair[0] == self.enum_name || (self.inside_enum_impl && pair[0] == "Self"))
                && self.variants.contains(&pair[1])
            {
                self.used.insert(pair[1].clone());
            }
        }
    }

    fn mark_use_tree(&mut self, tree: &UseTree, prefix: &mut Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.mark_use_tree(&path.tree, prefix);
                prefix.pop();
            }
            UseTree::Name(name) => {
                if prefix.last().is_some_and(|p| p == self.enum_name) {
                    let name = name.ident.to_string();
                    if self.variants.contains(&name) {
                        self.used.insert(name);
                    }
                }
            }
            UseTree::Rename(rename) => {
                if prefix.last().is_some_and(|p| p == self.enum_name) {
                    let name = rename.ident.to_string();
                    if self.variants.contains(&name) {
                        self.used.insert(name);
                    }
                }
            }
            UseTree::Glob(_) => {
                if prefix.last().is_some_and(|p| p == self.enum_name) {
                    self.used.extend(self.variants.iter().cloned());
                }
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.mark_use_tree(item, prefix);
                }
            }
        }
    }

    fn mark_macro_tokens(&mut self, tokens: TokenStream) {
        // We cannot expand procedural/declarative macros at analysis time.
        // Conservatively treat *any* mention of a matching variant identifier
        // inside a macro payload as use (false negatives over false positives).
        for token in tokens {
            match token {
                TokenTree::Ident(id) => {
                    let name = id.to_string();
                    if self.variants.contains(&name) {
                        self.used.insert(name);
                    }
                }
                TokenTree::Group(group) => self.mark_macro_tokens(group.stream()),
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for VariantUsage<'_> {
    fn visit_path(&mut self, path: &'ast Path) {
        self.mark_path(path);
        visit::visit_path(self, path);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let prior = self.inside_enum_impl;
        self.inside_enum_impl = matches!(
            &*item.self_ty,
            Type::Path(ty) if ty.path.segments.last().is_some_and(|s| s.ident == self.enum_name)
        );
        visit::visit_item_impl(self, item);
        self.inside_enum_impl = prior;
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        // Imports can be used under aliases. Match `use Error::{A, B}` and
        // `use crate::Error::*` structurally instead of regexing source text.
        self.mark_use_tree(&item.tree, &mut Vec::new());
        visit::visit_item_use(self, item);
    }

    fn visit_macro(&mut self, item: &'ast Macro) {
        self.mark_macro_tokens(item.tokens.clone());
        visit::visit_macro(self, item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_inline_golden_for_source_local_dead_variant() {
        let source = r#"
            #[contracterror]
            pub enum Error { Used = 1, Dead = 2 }
            fn work() -> Error { Error::Used }
        "#;
        let findings = DeadContracterrorVariantRule::new().check(source);
        let messages: Vec<_> = findings.into_iter().map(|v| v.message).collect();
        insta::assert_snapshot!(messages.join("\n"), @"Unused #[contracterror] variant Error::Dead (no source-local reference)");
    }

    #[test]
    fn macro_and_impl_references_are_not_reported() {
        let source = r#"
            #[contracterror]
            pub enum Error { FromMacro = 1, FromImpl = 2, FromArg = 3 }
            macro_rules! make_error { () => { Error::FromMacro } }
            make_error!();
            impl Error { fn fallback() -> Self { Self::FromImpl } }
            emit!(FromArg);
        "#;
        assert!(DeadContracterrorVariantRule::new().check(source).is_empty());
    }

    #[test]
    fn imports_and_non_contracterror_enums_are_conservative() {
        let source = r#"
            #[contracterror]
            pub enum Error { Imported, Glob }
            use Error::{Imported, Glob};
            pub enum Other { Dead }
        "#;
        assert!(DeadContracterrorVariantRule::new().check(source).is_empty());
    }
}
