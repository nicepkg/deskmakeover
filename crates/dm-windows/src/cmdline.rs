//! Windows command-line argument quoting (`CommandLineToArgvW` rules), split out cross-platform so
//! it compiles and is unit-tested on the Mac host. Used when the elevated-overlay client launches
//! `dm-elevated` via `ShellExecuteEx` `runas`: a `--file <path>` argument whose path contains a
//! space, a quote, or a trailing backslash must be encoded so the ELEVATED process's
//! `CommandLineToArgvW` parses it back as exactly one argument (ELEV-3 — a privileged launch must
//! never let a crafted path inject extra tokens into the helper's command line).

/// Quotes one argument per the `CommandLineToArgvW` / MSVCRT parsing rules (Daniel Colascione,
/// "Everyone quotes command line arguments the wrong way"). Backslashes are literal EXCEPT before a
/// double quote: a run of N backslashes before a `"` becomes `2N` backslashes plus `\"`; a run
/// before the closing quote becomes `2N` (so the quote stays a delimiter); an interior run stays
/// N. A non-empty argument free of space, tab, quote, or vertical-tab needs no quoting and is
/// returned verbatim. The empty argument becomes `""`.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '"' | '\u{0b}')) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                // The pending backslashes precede a quote → double them, then escape the quote.
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    // Trailing backslashes precede the closing quote → double them so it stays a delimiter.
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference `CommandLineToArgvW` parser (post-`argv[0]`): decode a command line into its FULL
    /// argument vector. Parsing everything — not just the first token (codex) — is what makes the
    /// injection test real: a trailing token that leaked past the quoting would show up as a SECOND
    /// element, so `round_trips`/`injection` assert exactly one argument comes back.
    fn parse_args(cmdline: &str) -> Vec<String> {
        let chars: Vec<char> = cmdline.chars().collect();
        let mut args = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1; // skip whitespace between arguments
            }
            if i >= chars.len() {
                break;
            }
            let mut out = String::new();
            let mut in_quotes = false;
            while i < chars.len() {
                let c = chars[i];
                if c == '\\' {
                    let mut n = 0;
                    while i < chars.len() && chars[i] == '\\' {
                        n += 1;
                        i += 1;
                    }
                    if i < chars.len() && chars[i] == '"' {
                        for _ in 0..n / 2 {
                            out.push('\\');
                        }
                        if n % 2 == 1 {
                            out.push('"'); // odd run → the quote is escaped (literal)
                            i += 1;
                        }
                        // even run → the quote stays a delimiter, handled next iteration
                    } else {
                        for _ in 0..n {
                            out.push('\\');
                        }
                    }
                } else if c == '"' {
                    in_quotes = !in_quotes;
                    i += 1;
                } else if (c == ' ' || c == '\t') && !in_quotes {
                    break; // end of this argument
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            args.push(out);
        }
        args
    }

    fn round_trips(arg: &str) {
        let quoted = quote_arg(arg);
        assert_eq!(
            parse_args(&quoted),
            vec![arg.to_string()],
            "quote_arg({arg:?}) = {quoted:?} did not round-trip to exactly one argument"
        );
    }

    #[test]
    fn plain_path_is_returned_verbatim() {
        assert_eq!(quote_arg(r"C:\gen\app.ico"), r"C:\gen\app.ico");
        round_trips(r"C:\gen\app.ico");
    }

    #[test]
    fn path_with_space_is_wrapped() {
        assert_eq!(quote_arg(r"C:\Program Files\a.ico"), r#""C:\Program Files\a.ico""#);
        round_trips(r"C:\Program Files\a.ico");
    }

    #[test]
    fn embedded_quote_is_escaped() {
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        round_trips(r#"a"b"#);
    }

    #[test]
    fn backslash_run_before_quote_is_doubled_plus_one() {
        // a \ " b  →  the backslash run before the quote doubles, then the quote is escaped.
        assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
        round_trips(r#"a\"b"#);
    }

    #[test]
    fn trailing_backslash_with_space_is_doubled() {
        // A space forces quoting; the trailing backslash must double so the closing quote survives.
        assert_eq!(quote_arg(r"C:\a b\"), r#""C:\a b\\""#);
        round_trips(r"C:\a b\");
    }

    #[test]
    fn injection_attempt_cannot_add_tokens() {
        // A path crafted to break the old bare-quote wrapping must round-trip to EXACTLY ONE
        // argument — no leaked --style/--file tokens the elevated helper would parse separately.
        let evil = r#"C:\x.ico" --style custom --file C:\evil.ico"#;
        round_trips(evil);
        let parsed = parse_args(&quote_arg(evil));
        assert_eq!(parsed, vec![evil.to_string()]);
        assert_eq!(parsed.len(), 1, "the crafted path must not split into multiple tokens");
    }

    #[test]
    fn empty_argument_becomes_empty_quotes() {
        assert_eq!(quote_arg(""), "\"\"");
        round_trips("");
    }

    #[test]
    fn tab_and_vertical_tab_force_quoting() {
        round_trips("a\tb");
        round_trips("a\u{0b}b");
    }
}
