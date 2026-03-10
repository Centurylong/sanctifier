use crate::recursion::{RecursionAnalyzer, RecursionType};

#[test]
fn test_direct_recursion_factorial() {
    let source = r#"
        impl Calculator {
            pub fn factorial(n: u32) -> u32 {
                if n <= 1 {
                    1
                } else {
                    n * factorial(n - 1)
                }
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].recursion_type, RecursionType::Direct);
    assert_eq!(issues[0].function_name, "factorial");
    assert!(issues[0].message.contains("calls itself directly"));
    assert_eq!(issues[0].call_chain, vec!["factorial", "factorial"]);
}

#[test]
fn test_direct_recursion_fibonacci() {
    let source = r#"
        impl Calculator {
            pub fn fibonacci(n: u32) -> u32 {
                if n <= 1 {
                    n
                } else {
                    fibonacci(n - 1) + fibonacci(n - 2)
                }
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].recursion_type, RecursionType::Direct);
    assert_eq!(issues[0].function_name, "fibonacci");
}

#[test]
fn test_indirect_recursion_two_functions() {
    let source = r#"
        impl MyContract {
            pub fn foo() {
                bar();
            }
            
            fn bar() {
                foo();
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(!issues.is_empty());
    // Should detect the cycle foo -> bar -> foo
    let issue = &issues[0];
    assert!(
        issue.recursion_type == RecursionType::Indirect 
        || issue.recursion_type == RecursionType::Direct
    );
    assert!(issue.call_chain.len() >= 2);
}

#[test]
fn test_indirect_recursion_three_functions() {
    let source = r#"
        impl MyContract {
            pub fn alpha() {
                beta();
            }
            
            fn beta() {
                gamma();
            }
            
            fn gamma() {
                alpha();
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(!issues.is_empty());
    let issue = &issues[0];
    assert_eq!(issue.recursion_type, RecursionType::Indirect);
    assert!(issue.call_chain.len() >= 3);
    assert!(issue.message.contains("recursive call chain"));
}

#[test]
fn test_no_recursion_linear_calls() {
    let source = r#"
        impl MyContract {
            pub fn process(value: u32) -> u32 {
                let x = validate(value);
                let y = transform(x);
                finalize(y)
            }
            
            fn validate(v: u32) -> u32 {
                v + 1
            }
            
            fn transform(v: u32) -> u32 {
                v * 2
            }
            
            fn finalize(v: u32) -> u32 {
                v - 1
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(issues.is_empty());
}

#[test]
fn test_no_recursion_simple_function() {
    let source = r#"
        impl MyContract {
            pub fn add(a: u32, b: u32) -> u32 {
                a + b
            }
            
            pub fn multiply(a: u32, b: u32) -> u32 {
                let sum = add(a, b);
                sum * 2
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(issues.is_empty());
}

#[test]
fn test_method_call_recursion() {
    let source = r#"
        impl Counter {
            pub fn count_down(&self, n: u32) {
                if n > 0 {
                    self.count_down(n - 1);
                }
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].function_name, "count_down");
    assert_eq!(issues[0].recursion_type, RecursionType::Direct);
}

#[test]
fn test_multiple_independent_recursions() {
    let source = r#"
        impl MyContract {
            pub fn factorial(n: u32) -> u32 {
                if n <= 1 {
                    1
                } else {
                    n * factorial(n - 1)
                }
            }
            
            pub fn fibonacci(n: u32) -> u32 {
                if n <= 1 {
                    n
                } else {
                    fibonacci(n - 1) + fibonacci(n - 2)
                }
            }
            
            pub fn add(a: u32, b: u32) -> u32 {
                a + b
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 2);
    let function_names: Vec<&str> = issues.iter()
        .map(|i| i.function_name.as_str())
        .collect();
    assert!(function_names.contains(&"factorial"));
    assert!(function_names.contains(&"fibonacci"));
}

#[test]
fn test_complex_indirect_recursion() {
    let source = r#"
        impl MyContract {
            pub fn process_a(n: u32) {
                if n > 0 {
                    helper_b(n);
                }
            }
            
            fn helper_b(n: u32) {
                if n > 1 {
                    helper_c(n - 1);
                }
            }
            
            fn helper_c(n: u32) {
                process_a(n);
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(!issues.is_empty());
    let issue = &issues[0];
    assert_eq!(issue.recursion_type, RecursionType::Indirect);
    // The cycle should be: process_a -> helper_b -> helper_c -> process_a
    assert!(issue.call_chain.len() >= 3);
}

#[test]
fn test_soroban_contract_with_recursion() {
    let source = r#"
        #[contractimpl]
        impl TokenContract {
            pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                from.require_auth();
                self.internal_transfer(env, from, to, amount);
            }
            
            fn internal_transfer(env: Env, from: Address, to: Address, amount: i128) {
                if amount > 1000 {
                    // Split large transfers
                    self.internal_transfer(env.clone(), from.clone(), to.clone(), amount / 2);
                    self.internal_transfer(env, from, to, amount / 2);
                }
            }
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].function_name, "internal_transfer");
    assert_eq!(issues[0].recursion_type, RecursionType::Direct);
}

#[test]
fn test_empty_source() {
    let source = "";

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert!(issues.is_empty());
}

#[test]
fn test_invalid_syntax() {
    let source = "this is not valid rust code {{{";

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    // Should handle gracefully and return empty
    assert!(issues.is_empty());
}

#[test]
fn test_standalone_functions() {
    let source = r#"
        fn recursive_helper(n: u32) -> u32 {
            if n == 0 {
                0
            } else {
                recursive_helper(n - 1)
            }
        }
        
        fn non_recursive(x: u32) -> u32 {
            x * 2
        }
    "#;

    let mut analyzer = RecursionAnalyzer::new();
    let issues = analyzer.analyze(source);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].function_name, "recursive_helper");
}
