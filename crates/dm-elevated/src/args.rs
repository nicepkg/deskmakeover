//! Pure argument parsing for the elevated helper. Ported from `ElevatedHelper/Program.cs`
//! (`GetOption`) and the style whitelist in `OverlayCommands.Apply`. Unit-tested on the host.

/// The overlay style. The whitelist is the security boundary: anything else is rejected before
/// any privileged action (oracle: `OverlayCommands.Apply` returns exit 2 on an unknown style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Refined,
    Transparent,
    Custom,
}

impl Style {
    pub fn parse(value: &str) -> Option<Style> {
        match value.trim().to_ascii_lowercase().as_str() {
            "refined" => Some(Style::Refined),
            "transparent" => Some(Style::Transparent),
            "custom" => Some(Style::Custom),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Style::Refined => "refined",
            Style::Transparent => "transparent",
            Style::Custom => "custom",
        }
    }
}

/// A parsed helper command (the fixed verb set — no arbitrary commands, ADR-0021 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    None,
    Version,
    ApplyOverlay { style: Style, file: Option<String> },
    RestoreOverlay,
    /// An unknown or non-whitelisted verb → rejected (exit 2).
    Unknown(String),
}

/// Parses argv (the program name already stripped). This is a PRIVILEGE BOUNDARY (audit F6), so the
/// grammar is STRICT: an unknown flag, a duplicate flag, a dangling flag, a surplus argument, or a
/// non-whitelisted `--style` value all reject to `Command::Unknown` (→ exit 2). Only a MISSING
/// `--style` keeps the `Refined` default; a PRESENT-but-invalid one is refused, never silently
/// downgraded. The C# helper's lenient `GetOption` is deliberately NOT mirrored (it is being retired).
pub fn parse(args: &[String]) -> Command {
    let Some(command) = args.first() else {
        return Command::None;
    };
    match command.trim().to_ascii_lowercase().as_str() {
        "version" if args.len() == 1 => Command::Version,
        "version" => Command::Unknown("version takes no arguments".into()),
        "apply-overlay" => parse_apply_overlay(&args[1..]),
        "restore-overlay" if args.len() == 1 => Command::RestoreOverlay,
        "restore-overlay" => Command::Unknown("restore-overlay takes no arguments".into()),
        other => Command::Unknown(other.to_string()),
    }
}

/// Strict `apply-overlay` grammar: only `--style <whitelisted>` and `--file <value>`, each at most
/// once, each requiring its value; anything else refuses.
fn parse_apply_overlay(rest: &[String]) -> Command {
    let mut style: Option<Style> = None;
    let mut file: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].to_ascii_lowercase().as_str() {
            "--style" => {
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v)) else {
                    return Command::Unknown("apply-overlay: --style needs a value".into());
                };
                if style.is_some() {
                    return Command::Unknown("apply-overlay: duplicate --style".into());
                }
                let Some(parsed) = Style::parse(val) else {
                    return Command::Unknown(format!("apply-overlay: unknown style {val:?}"));
                };
                style = Some(parsed);
                i += 2;
            }
            "--file" => {
                // A flag-looking or empty next token is NOT a value (codex B4-🟡): `--file --style`
                // must reject as a missing value (exit 2), not silently take "--style" as the path.
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v) && !v.trim().is_empty()) else {
                    return Command::Unknown("apply-overlay: --file needs a value".into());
                };
                if file.is_some() {
                    return Command::Unknown("apply-overlay: duplicate --file".into());
                }
                file = Some(val.trim().to_string());
                i += 2;
            }
            other => return Command::Unknown(format!("apply-overlay: unexpected argument {other:?}")),
        }
    }
    Command::ApplyOverlay { style: style.unwrap_or(Style::Refined), file }
}

/// Whether `token` looks like an option flag, so it can never be consumed as an option's value.
fn is_flag(token: &str) -> bool {
    token.starts_with("--")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_the_fixed_verb_set() {
        assert_eq!(parse(&argv(&[])), Command::None);
        assert_eq!(parse(&argv(&["version"])), Command::Version);
        assert_eq!(parse(&argv(&["restore-overlay"])), Command::RestoreOverlay);
        assert_eq!(parse(&argv(&["do-something-evil"])), Command::Unknown("do-something-evil".into()));
    }

    #[test]
    fn apply_overlay_reads_style_and_file() {
        let cmd = parse(&argv(&["apply-overlay", "--style", "transparent", "--file", r"C:\gen\clear.ico"]));
        assert_eq!(cmd, Command::ApplyOverlay { style: Style::Transparent, file: Some(r"C:\gen\clear.ico".into()) });
    }

    #[test]
    fn present_but_invalid_style_is_rejected_not_downgraded() {
        // A bogus style value REFUSES (privilege boundary) rather than silently becoming Refined.
        assert!(matches!(parse(&argv(&["apply-overlay", "--style", "rm-rf"])), Command::Unknown(_)));
    }

    #[test]
    fn missing_style_keeps_the_refined_default() {
        assert_eq!(parse(&argv(&["apply-overlay"])), Command::ApplyOverlay { style: Style::Refined, file: None });
    }

    #[test]
    fn file_value_case_is_preserved() {
        let cmd = parse(&argv(&["apply-overlay", "--style", "custom", "--file", r"C:\Users\Jane\Icon.ICO"]));
        match cmd {
            Command::ApplyOverlay { file: Some(f), .. } => assert_eq!(f, r"C:\Users\Jane\Icon.ICO"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn verb_matching_is_case_insensitive_and_trimmed() {
        assert_eq!(parse(&argv(&["  VERSION "])), Command::Version);
        assert_eq!(parse(&argv(&["Restore-Overlay"])), Command::RestoreOverlay);
    }

    #[test]
    fn injection_style_verbs_are_rejected_as_unknown() {
        // The verb is the whole first arg — no shell, no chaining. These are NOT whitelisted.
        for evil in ["apply-overlay && calc.exe", "restore-overlay; rm -rf /", "../../evil", "apply-overlay --file x"] {
            assert!(matches!(parse(&argv(&[evil])), Command::Unknown(_)), "{evil} must be Unknown");
        }
    }

    #[test]
    fn dangling_flags_and_unknown_arguments_are_rejected() {
        // A privilege boundary refuses anything it does not fully understand.
        assert!(matches!(parse(&argv(&["apply-overlay", "--style", "transparent", "--file"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "--style"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "--unexpected", "v", "--file", "x.ico"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "surplus"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["version", "extra"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["restore-overlay", "x"])), Command::Unknown(_)));
    }

    #[test]
    fn a_flag_looking_or_empty_value_is_not_consumed() {
        // `--file --style` must reject (missing value), not take "--style" as the path (codex B4-🟡).
        assert!(matches!(parse(&argv(&["apply-overlay", "--file", "--style", "custom"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "--style", "--file", "x.ico"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "--file", ""])), Command::Unknown(_)));
    }

    #[test]
    fn duplicate_flags_are_rejected() {
        assert!(matches!(parse(&argv(&["apply-overlay", "--file", "first.ico", "--file", "second.ico"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["apply-overlay", "--style", "custom", "--style", "refined"])), Command::Unknown(_)));
    }

    #[test]
    fn style_whitelist_is_case_insensitive_but_closed() {
        assert_eq!(Style::parse("CUSTOM"), Some(Style::Custom));
        assert_eq!(Style::parse("  Transparent "), Some(Style::Transparent));
        for bogus in ["", "custom;drop", "reg", "REFINED_MARK", "../x"] {
            assert_eq!(Style::parse(bogus), None, "{bogus} must not parse");
        }
    }
}
