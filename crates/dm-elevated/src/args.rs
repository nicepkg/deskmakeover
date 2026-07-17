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
    /// Batch-write desktop-item icons that the unelevated app could not (a Public/All-Users desktop
    /// `.lnk`, folder `desktop.ini`, etc.). The single arg is a MANIFEST file path; the helper reads
    /// it, independently validates every target (never trusting the manifest), and writes atomically.
    ApplyDesktopItems { manifest: String },
    /// Restore those same protected targets to their captured originals (the reverse batch).
    RestoreDesktopItems { manifest: String },
    /// Run as a SESSION-SCOPED elevated server: create the named pipe `pipe`, accept ONLY the
    /// unelevated app process identified by BOTH `client_pid` AND `client_created` (the launcher's
    /// process-creation FILETIME as a u64), and execute the SAME whitelisted verbs above — one UAC per
    /// app launch instead of one per operation (owner 2026-07-17). The (pid, creation-time) pair is
    /// the identity: a bare pid is forgeable by PID reuse, so the server verifies the connecting (and
    /// watched) process's `GetProcessTimes` creation time equals `client_created` before trusting it
    /// (codex 2026-07-17 P1). Every request string is re-parsed through THIS grammar, so the pipe
    /// grants no capability the CLI does not. The server exits when the client dies (never a lingering
    /// elevated process).
    ServeSession { pipe: String, client_pid: u32, client_created: u64 },
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
        "apply-desktop-items" => parse_desktop_items(true, &args[1..]),
        "restore-desktop-items" => parse_desktop_items(false, &args[1..]),
        "serve-session" => parse_serve_session(&args[1..]),
        other => Command::Unknown(other.to_string()),
    }
}

/// Strict `serve-session` grammar: `--pipe <name>` and `--client-pid <u32>`, each exactly once,
/// each with a non-flag, non-empty value; `client-pid` must parse as a non-zero u32. Anything else
/// refuses (exit 2). The pipe NAME is a single path segment the server suffixes onto `\\.\pipe\`
/// (never interpolated elsewhere); a value containing a separator or `..` is rejected here so it
/// can never escape the pipe namespace.
fn parse_serve_session(rest: &[String]) -> Command {
    let mut pipe: Option<String> = None;
    let mut client_pid: Option<u32> = None;
    let mut client_created: Option<u64> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].to_ascii_lowercase().as_str() {
            "--pipe" => {
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v) && !v.trim().is_empty()) else {
                    return Command::Unknown("serve-session: --pipe needs a value".into());
                };
                if pipe.is_some() {
                    return Command::Unknown("serve-session: duplicate --pipe".into());
                }
                let name = val.trim();
                if !is_safe_pipe_name(name) {
                    return Command::Unknown("serve-session: --pipe is not a safe single segment".into());
                }
                pipe = Some(name.to_string());
                i += 2;
            }
            "--client-pid" => {
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v)) else {
                    return Command::Unknown("serve-session: --client-pid needs a value".into());
                };
                if client_pid.is_some() {
                    return Command::Unknown("serve-session: duplicate --client-pid".into());
                }
                match val.trim().parse::<u32>() {
                    Ok(p) if p != 0 => client_pid = Some(p),
                    _ => return Command::Unknown("serve-session: --client-pid must be a non-zero u32".into()),
                }
                i += 2;
            }
            "--client-created" => {
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v)) else {
                    return Command::Unknown("serve-session: --client-created needs a value".into());
                };
                if client_created.is_some() {
                    return Command::Unknown("serve-session: duplicate --client-created".into());
                }
                match val.trim().parse::<u64>() {
                    Ok(t) if t != 0 => client_created = Some(t),
                    _ => return Command::Unknown("serve-session: --client-created must be a non-zero u64".into()),
                }
                i += 2;
            }
            other => return Command::Unknown(format!("serve-session: unexpected argument {other:?}")),
        }
    }
    match (pipe, client_pid, client_created) {
        (Some(pipe), Some(client_pid), Some(client_created)) => {
            Command::ServeSession { pipe, client_pid, client_created }
        }
        _ => Command::Unknown(
            "serve-session: --pipe, --client-pid and --client-created are all required".into(),
        ),
    }
}

/// A safe named-pipe leaf: non-empty, only `[A-Za-z0-9._-]`, no separators / `..` so it stays one
/// segment under `\\.\pipe\`.
fn is_safe_pipe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Strict `apply|restore-desktop-items` grammar: only `--manifest <value>`, exactly once, with a
/// non-flag, non-empty value. Everything else refuses (exit 2). The manifest's CONTENT is validated
/// later by the batch itself — this only guards the command line.
fn parse_desktop_items(apply: bool, rest: &[String]) -> Command {
    let verb = if apply { "apply-desktop-items" } else { "restore-desktop-items" };
    let mut manifest: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].to_ascii_lowercase().as_str() {
            "--manifest" => {
                let Some(val) = rest.get(i + 1).filter(|v| !is_flag(v) && !v.trim().is_empty()) else {
                    return Command::Unknown(format!("{verb}: --manifest needs a value"));
                };
                if manifest.is_some() {
                    return Command::Unknown(format!("{verb}: duplicate --manifest"));
                }
                manifest = Some(val.trim().to_string());
                i += 2;
            }
            other => return Command::Unknown(format!("{verb}: unexpected argument {other:?}")),
        }
    }
    match manifest {
        Some(manifest) if apply => Command::ApplyDesktopItems { manifest },
        Some(manifest) => Command::RestoreDesktopItems { manifest },
        None => Command::Unknown(format!("{verb}: --manifest is required")),
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

    #[test]
    fn serve_session_requires_a_safe_pipe_a_nonzero_pid_and_a_creation_time() {
        assert_eq!(
            parse(&argv(&["serve-session", "--pipe", "dm-abc123", "--client-pid", "4242", "--client-created", "133700000000000000"])),
            Command::ServeSession { pipe: "dm-abc123".into(), client_pid: 4242, client_created: 133_700_000_000_000_000 }
        );
        // Strict privilege-boundary grammar: all three are required (the (pid, creation-time) pair is
        // the identity — a bare pid is forgeable, so `--client-created` is mandatory).
        assert!(matches!(parse(&argv(&["serve-session"])), Command::Unknown(_)), "all required");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-pid", "5"])), Command::Unknown(_)), "created required");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-created", "1"])), Command::Unknown(_)), "pid required");
        assert!(matches!(parse(&argv(&["serve-session", "--client-pid", "5", "--client-created", "1"])), Command::Unknown(_)), "pipe required");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-pid", "0", "--client-created", "1"])), Command::Unknown(_)), "pid nonzero");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-pid", "-1", "--client-created", "1"])), Command::Unknown(_)), "pid u32");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-pid", "5", "--client-created", "0"])), Command::Unknown(_)), "created nonzero");
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "x", "--client-pid", "5", "--client-created", "notnum"])), Command::Unknown(_)), "created numeric");
        // Pipe name must be a safe single segment — no path escape into the pipe namespace.
        for bad in [r"..\evil", r"a\b", "a/b", "", ".", "..", "has space", "semi;colon"] {
            assert!(
                matches!(parse(&argv(&["serve-session", "--pipe", bad, "--client-pid", "5", "--client-created", "1"])), Command::Unknown(_)),
                "pipe {bad:?} must be rejected"
            );
        }
        // Duplicates + surplus refuse.
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "a", "--pipe", "b", "--client-pid", "5", "--client-created", "1"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "a", "--client-pid", "5", "--client-created", "1", "--client-created", "2"])), Command::Unknown(_)));
        assert!(matches!(parse(&argv(&["serve-session", "--pipe", "a", "--client-pid", "5", "--client-created", "1", "surplus"])), Command::Unknown(_)));
    }

    #[test]
    fn desktop_items_verbs_require_exactly_one_manifest() {
        assert_eq!(
            parse(&argv(&["apply-desktop-items", "--manifest", r"C:\tmp\m.txt"])),
            Command::ApplyDesktopItems { manifest: r"C:\tmp\m.txt".into() }
        );
        assert_eq!(
            parse(&argv(&["restore-desktop-items", "--manifest", r"C:\tmp\m.txt"])),
            Command::RestoreDesktopItems { manifest: r"C:\tmp\m.txt".into() }
        );
        // Strict privilege-boundary grammar: missing/empty/dangling/duplicate/surplus all refuse.
        assert!(matches!(parse(&argv(&["apply-desktop-items"])), Command::Unknown(_)), "missing --manifest");
        assert!(matches!(parse(&argv(&["apply-desktop-items", "--manifest"])), Command::Unknown(_)), "dangling");
        assert!(matches!(parse(&argv(&["apply-desktop-items", "--manifest", ""])), Command::Unknown(_)), "empty");
        assert!(
            matches!(parse(&argv(&["apply-desktop-items", "--manifest", "a", "--manifest", "b"])), Command::Unknown(_)),
            "duplicate"
        );
        assert!(
            matches!(parse(&argv(&["restore-desktop-items", "--manifest", "a", "surplus"])), Command::Unknown(_)),
            "surplus positional"
        );
        assert!(
            matches!(parse(&argv(&["apply-desktop-items", "--evil", "x"])), Command::Unknown(_)),
            "unknown flag"
        );
    }
}
