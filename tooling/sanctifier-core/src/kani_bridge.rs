use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{parse_file, parse_quote, Expr, File, Item, ItemImpl};

/// A mutable visitor that traverses the AST and replaces Soroban
/// env.storage().*.set/get calls with a dummy Kani state representation.
/// For simplicity in this POC bridge, we strip `env` references
/// and replace them with standard mocked results that Kani can verify.
struct KaniBridgeVisitor {
    in_contract_impl: bool,
}

impl VisitMut for KaniBridgeVisitor {
    fn visit_item_impl_mut(&mut self, i: &mut ItemImpl) {
        // Check if this impl has the #[contractimpl] attribute
        let is_contract = i.attrs.iter().any(|attr| {
            if let syn::Meta::Path(path) = &attr.meta {
                path.is_ident("contractimpl")
            } else {
                false
            }
        });

        if is_contract {
            self.in_contract_impl = true;
            syn::visit_mut::visit_item_impl_mut(self, i);
            self.in_contract_impl = false;
        } else {
            syn::visit_mut::visit_item_impl_mut(self, i);
        }
    }

    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if !self.in_contract_impl {
            syn::visit_mut::visit_expr_mut(self, expr);
            return;
        }

        // We want to replace `env.storage().instance().set(...)`
        // with `// mocked by sanctifier-kani`.
        // In a full implementation, we would rewrite this into a formal Map.
        // For this bridge, we find common Soroban host calls and stub them.
        if let Expr::MethodCall(mc) = expr {
            let method_name = mc.method.to_string();

            // Very naive check for `env.storage()`
            let receiver_str = quote!(#mc).to_string();
            if (receiver_str.contains("env . storage ( )")
                || receiver_str.contains("env.storage()"))
                && (method_name == "set" || method_name == "get" || method_name == "has")
            {
                // Replace the whole expression with a Kani mock
                if method_name == "get" || method_name == "has" {
                    *expr = parse_quote!(kani::any());
                } else {
                    // For void returns like set, remove
                    *expr = parse_quote!(());
                }
                return;
            }
        }

        syn::visit_mut::visit_expr_mut(self, expr);
    }
}

pub struct KaniBridge;

impl KaniBridge {
    /// Takes a Soroban contract source string and returns a Kani-compatible
    /// Rust source string with host environment calls mocked.
    pub fn translate_for_kani(source: &str, unwind: Option<u32>) -> Result<String, syn::Error> {
        let mut ast: File = parse_file(source)?;

        let mut visitor = KaniBridgeVisitor {
            in_contract_impl: false,
        };
        visitor.visit_file_mut(&mut ast);

        // Inject Kani harness generation
        // For every public method in a `#[contractimpl]`, we generate a Kani proof.
        let mut harnesses = Vec::new();

        for item in &ast.items {
            if let Item::Impl(i) = item {
                // If this is the contract impl
                if i.attrs
                    .iter()
                    .any(|attr| attr.meta.path().is_ident("contractimpl"))
                {
                    for impl_item in &i.items {
                        if let syn::ImplItem::Fn(f) = impl_item {
                            if let syn::Visibility::Public(_) = f.vis {
                                let fn_name = &f.sig.ident;
                                let harness_name = quote::format_ident!("kani_harness_{}", fn_name);

                                // Extract args to generate kani::any() for them
                                // (Skipping 'env: Env' if present)
                                let mut kani_let_statements = Vec::new();
                                let mut call_args = Vec::new();

                                for arg in &f.sig.inputs {
                                    if let syn::FnArg::Typed(pat_type) = arg {
                                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                            let arg_name = &pat_ident.ident;
                                            let arg_ty = &pat_type.ty;

                                            // Skip Env
                                            let ty_str = quote!(#arg_ty).to_string();
                                            if ty_str != "Env" && ty_str != "soroban_sdk :: Env" {
                                                kani_let_statements.push(quote! {
                                                    let #arg_name : #arg_ty = kani::any();
                                                });
                                                call_args.push(quote!(#arg_name));
                                            } else {
                                                call_args.push(quote!(soroban_sdk::Env::default()));
                                            }
                                        }
                                    }
                                }

                                let self_ty = &i.self_ty;

                                // Inject loop unwinding if specified
                                let unwind_attr = if let Some(n) = unwind {
                                    quote! { #[kani::unwind(#n)] }
                                } else {
                                    quote! {}
                                };

                                let harness = quote! {
                                    #[cfg(kani)]
                                    #[kani::proof]
                                    #unwind_attr
                                    pub fn #harness_name() {
                                        #(#kani_let_statements)*
                                        // Mock the call
                                        let _ = #self_ty :: #fn_name ( #(#call_args),* );
                                    }
                                };
                                harnesses.push(harness);
                            }
                        }
                    }
                }
            }
        }

        let generated_str = quote!(#ast).to_string();
        let harnesses_str = quote!(#(#harnesses)*).to_string();

        // Append harnesses at the end
        Ok(format!("{}\n{}", generated_str, harnesses_str))
    }
}
#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_kani_unwind_injection() {
        let source = r#"
            #[contractimpl]
            impl MyContract {
                pub fn loop_fn(env: Env, n: u32) {
                    for i in 0..n {
                        // some loop
                    }
                }
            }
        "#;

        // Test with unwind = Some(5)
        let result = KaniBridge::translate_for_kani(source, Some(5)).unwrap();
        // quote! might format numbers with type suffixes in string output
        assert!(result.contains("kani :: unwind (5)") || result.contains("kani :: unwind (5u32)"));
        assert!(result.contains("kani :: proof"));

        // Test with unwind = None
        let result_no_unwind = KaniBridge::translate_for_kani(source, None).unwrap();
        assert!(!result_no_unwind.contains("kani :: unwind"));
    }
}
