use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashMap;
use syn::visit::Visit;
use syn::{File, ItemFn, ReturnType, Type, Visibility};

pub struct ContracterrorEnumRule;

impl ContracterrorEnumRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContracterrorEnumRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ContracterrorEnumRule {
    fn name(&self) -> &str {
        "contracterror_enum"
    }

    fn description(&self) -> &str {
        "Detects missing #[contracterror] or unstable repr on error enums returned from public functions"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
            None => return vec![],
        };

        let mut visitor = ContractErrorVisitor {
            issues: Vec::new(),
            enums: HashMap::new(),
        };
        visitor.visit_file(&file);

        visitor
            .issues
            .into_iter()
            .map(|issue| {
                RuleViolation::new(
                    "SANCT_CONTRACTERROR_ENUM",
                    Severity::Warning,
                    issue.message,
                    issue.location,
                )
                .with_suggestion(
                    "Add #[contracterror] and a stable #[repr(...)] attribute to the error enum"
                        .to_string(),
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ContractErrorVisitor {
    issues: Vec<ContractErrorIssue>,
    enums: HashMap<String, EnumInfo>,
}

struct ContractErrorIssue {
    message: String,
    location: String,
}

#[derive(Clone)]
struct EnumInfo {
    has_contracterror: bool,
    has_repr: bool,
}

impl<'ast> Visit<'ast> for ContractErrorVisitor {
    fn visit_file(&mut self, node: &'ast File) {
        for item in &node.items {
            if let syn::Item::Enum(e) = item {
                let name = e.ident.to_string();
                let mut has_contracterror = false;
                let mut has_repr = false;

                for attr in &e.attrs {
                    if attr.path().is_ident("contracterror") {
                        has_contracterror = true;
                    } else if attr.path().is_ident("repr") {
                        has_repr = true;
                    }
                }

                self.enums.insert(
                    name,
                    EnumInfo {
                        has_contracterror,
                        has_repr,
                    },
                );
            }
        }

        syn::visit::visit_file(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if matches!(node.vis, Visibility::Public(_)) {
            if let ReturnType::Type(_, ty) = &node.sig.output {
                if let Type::Path(tp) = &**ty {
                    if let Some(seg) = tp.path.segments.last() {
                        if seg.ident == "Result" {
                            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                                if args.args.len() == 2 {
                                    if let syn::GenericArgument::Type(Type::Path(err_tp)) =
                                        &args.args[1]
                                    {
                                        if let Some(err_seg) = err_tp.path.segments.last() {
                                            let err_name = err_seg.ident.to_string();
                                            if let Some(enum_info) = self.enums.get(&err_name) {
                                                if !enum_info.has_contracterror
                                                    || !enum_info.has_repr
                                                {
                                                    let line = node.sig.ident.span().start().line;
                                                    let mut msg = Vec::new();
                                                    if !enum_info.has_contracterror {
                                                        msg.push("missing #[contracterror]");
                                                    }
                                                    if !enum_info.has_repr {
                                                        msg.push("missing stable #[repr(...)]");
                                                    }
                                                    self.issues.push(ContractErrorIssue {
                                                        message: format!(
                                                            "Error enum '{}' returned from public function '{}' is {}",
                                                            err_name,
                                                            node.sig.ident,
                                                            msg.join(" and ")
                                                        ),
                                                        location: format!("{}:{}", node.sig.ident, line),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        syn::visit::visit_item_fn(self, node);
    }
}
