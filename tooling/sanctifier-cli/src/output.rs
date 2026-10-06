use colored::control;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorDecision {
    Auto,
    Plain,
}

pub fn configure_from_env() {
    let no_color = env::var_os("NO_COLOR").is_some();
    let ci = env::var_os("CI")
        .map(|value| ci_value_is_truthy(&value.to_string_lossy()))
        .unwrap_or(false);
    let theme = env::var("SANCTIFIER_THEME").unwrap_or_default();

    match resolve_color_decision(no_color, ci, &theme) {
        ColorDecision::Auto => control::unset_override(),
        ColorDecision::Plain => control::set_override(false),
    }
}

fn resolve_color_decision(no_color: bool, ci: bool, theme: &str) -> ColorDecision {
    if no_color || ci || is_plain_theme(theme) {
        ColorDecision::Plain
    } else {
        ColorDecision::Auto
    }
}

fn is_plain_theme(theme: &str) -> bool {
    matches!(
        theme.trim().to_ascii_lowercase().as_str(),
        "plain" | "mono" | "monochrome"
    )
}

fn ci_value_is_truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_disables_styling_even_with_classic_theme() {
        assert_eq!(resolve_color_decision(true, false, "classic"), ColorDecision::Plain);
    }

    #[test]
    fn ci_disables_styling_for_log_safe_output() {
        assert_eq!(resolve_color_decision(false, true, "classic"), ColorDecision::Plain);
    }

    #[test]
    fn plain_theme_is_an_explicit_monochrome_option() {
        assert_eq!(resolve_color_decision(false, false, "plain"), ColorDecision::Plain);
        assert_eq!(resolve_color_decision(false, false, "classic"), ColorDecision::Auto);
    }

    #[test]
    fn explicit_false_ci_values_do_not_disable_interactive_color() {
        for value in ["", "0", "false", "no", "off", " FALSE "] {
            assert!(!ci_value_is_truthy(value), "{value:?} should be false");
        }
        assert!(ci_value_is_truthy("true"));
        assert!(ci_value_is_truthy("1"));
    }
}
