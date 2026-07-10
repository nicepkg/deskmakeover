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

/// Parses argv (the program name already stripped).
pub fn parse(args: &[String]) -> Command {
    let Some(command) = args.first() else {
        return Command::None;
    };
    match command.trim().to_ascii_lowercase().as_str() {
        "version" => Command::Version,
        "apply-overlay" => {
            let style = option(args, "--style")
                .and_then(|s| Style::parse(&s))
                .unwrap_or(Style::Refined);
            Command::ApplyOverlay { style, file: option(args, "--file") }
        }
        "restore-overlay" => Command::RestoreOverlay,
        other => Command::Unknown(other.to_string()),
    }
}

/// Finds `--name <value>` after the command. Unlike the oracle (which lowercased every option
/// value), the raw value is preserved so a case-sensitive `--file` path is not corrupted.
fn option(args: &[String], name: &str) -> Option<String> {
    if args.len() < 2 {
        return None;
    }
    for i in 1..args.len() - 1 {
        if args[i].eq_ignore_ascii_case(name) {
            return Some(args[i + 1].trim().to_string());
        }
    }
    None
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
    fn unknown_style_falls_back_to_refined_default() {
        // A bogus style is not honoured; the default is used (the whitelist still gates the value).
        let cmd = parse(&argv(&["apply-overlay", "--style", "rm-rf"]));
        assert_eq!(cmd, Command::ApplyOverlay { style: Style::Refined, file: None });
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
    fn trailing_option_flag_without_a_value_is_ignored() {
        // `--file` as the very last token has no value → None (oracle scans 1..len-1).
        let cmd = parse(&argv(&["apply-overlay", "--style", "transparent", "--file"]));
        assert_eq!(cmd, Command::ApplyOverlay { style: Style::Transparent, file: None });
        // `--style` with no value → falls back to the default.
        let cmd2 = parse(&argv(&["apply-overlay", "--style"]));
        assert_eq!(cmd2, Command::ApplyOverlay { style: Style::Refined, file: None });
    }

    #[test]
    fn first_matching_option_wins() {
        let cmd = parse(&argv(&["apply-overlay", "--file", "first.ico", "--file", "second.ico"]));
        match cmd {
            Command::ApplyOverlay { file: Some(f), .. } => assert_eq!(f, "first.ico"),
            other => panic!("unexpected {other:?}"),
        }
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
