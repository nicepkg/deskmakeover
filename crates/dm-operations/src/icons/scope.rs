//! Privileged-scope classification (spec 07 §6/§14) — the shared hard gate that keeps automation
//! (incremental auto-format, version switching, reset) off `Public Desktop` / `ProgramData`. A
//! pure path predicate so BOTH the resident reconciler and the operations-layer version switch
//! classify identically; the host resolves the real roots via `SHGetKnownFolderPath` and injects
//! them (never hardcoded).

/// Why a path is privileged (needs elevation / is not the user's own desktop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegedScope {
    /// Under the shared `Public Desktop` root.
    PublicDesktop,
    /// Under `ProgramData` (installer-deployed).
    ProgramData,
    /// The platform requires privileged roots but they are unresolved ([`ScopeRoots::Unresolved`]).
    /// Every item is treated as privileged so automation FAILS CLOSED — it touches nothing until the
    /// host resolves the real known folders (spec 07 §14). Never a benign "not privileged".
    Undetermined,
}

/// Normalizes a path for case-insensitive component comparison: lowercase (NTFS is
/// case-insensitive) + forward slashes + FOLDED `.`/`..` against the drive/UNC anchor + no
/// trailing separator (codex r3-🔴: a raw text prefix compare mis-handles traversal —
/// `…/Desktop/../Private` would false-match `…/Desktop`, and `…/DesktopBackup/../Desktop/x` would
/// false-MISS the real privileged dir). `..` that would escape the anchor is dropped (it cannot
/// go above a drive root), so it can never be used to slip a component past the gate.
fn normalize(path: &str) -> String {
    let lower = path.to_ascii_lowercase().replace('\\', "/");
    // Split the anchor (drive `c:` or UNC `//server/share`) from the rest so `..` folds only
    // within the path, never above the root.
    let (anchor, rest) = split_anchor(&lower);
    let mut stack: Vec<&str> = Vec::new();
    for comp in rest.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                stack.pop(); // escaping the anchor is impossible — a no-op at the root
            }
            other => stack.push(other),
        }
    }
    let joined = stack.join("/");
    if anchor.is_empty() {
        joined
    } else if joined.is_empty() {
        anchor
    } else {
        format!("{anchor}/{joined}")
    }
}

/// Splits `//server/share/rest` or `c:/rest` into `(anchor, rest)`; a non-rooted path has an
/// empty anchor. The anchor is never subject to `..` folding.
fn split_anchor(lower: &str) -> (String, &str) {
    if let Some(after) = lower.strip_prefix("//") {
        // UNC: anchor = //server/share
        let mut it = after.splitn(3, '/');
        let server = it.next().unwrap_or("");
        let share = it.next().unwrap_or("");
        let rest = it.next().unwrap_or("");
        if !server.is_empty() && !share.is_empty() {
            return (format!("//{server}/{share}"), rest);
        }
        return (String::new(), lower);
    }
    // Drive: c:/rest
    if lower.as_bytes().get(1) == Some(&b':') {
        let (drive, rest) = lower.split_at(2);
        return (drive.to_string(), rest.trim_start_matches('/'));
    }
    (String::new(), lower)
}

/// True when `path` is `root` itself or a descendant of it — a real DIRECTORY-ancestry test, not a
/// bare `starts_with` (codex m7b-🟠3: `…/Desktop` must NOT match `…/DesktopBackup`, and
/// `…/Public` must NOT match `…/PublicX`). Both are pre-normalized.
fn is_within(path_norm: &str, root: &str) -> bool {
    let root_norm = normalize(root);
    if root_norm.is_empty() {
        return false;
    }
    path_norm == root_norm || path_norm.starts_with(&format!("{root_norm}/"))
}

/// Classifies `path`'s write scope against the privileged roots the host resolved. `public_roots`
/// are the per-user + all-users `Public Desktop` known folders; `programdata_roots` the
/// `ProgramData` known folder(s). Ancestry is by path COMPONENT, so a sibling with a shared name
/// prefix never false-matches.
pub fn privileged_scope(
    path: &str,
    public_roots: &[String],
    programdata_roots: &[String],
) -> Option<PrivilegedScope> {
    let norm = normalize(path);
    if public_roots.iter().any(|r| is_within(&norm, r)) {
        return Some(PrivilegedScope::PublicDesktop);
    }
    if programdata_roots.iter().any(|r| is_within(&norm, r)) {
        return Some(PrivilegedScope::ProgramData);
    }
    None
}

/// Constructing [`ScopeRoots::resolved`] with a degenerate (all-empty) known-folder list — which on
/// Windows means resolution FAILED, and running with it would fail OPEN (every `Public Desktop`
/// item classified as the user's own). Callers turn this into [`ScopeRoots::Unresolved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyScopeRoots;

/// The privileged-scope roots for THIS platform, modelled so the fail-OPEN state — "a platform that
/// HAS a privileged desktop scope, but with empty roots" — is UNREPRESENTABLE. Automated write paths
/// (version switch, reset, resident auto-format) take a `&ScopeRoots` and call [`classify`], so the
/// §14 red-line cannot be silently bypassed by a host that forgot to resolve its known folders.
///
/// [`classify`]: ScopeRoots::classify
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeRoots {
    /// This platform has no shared/privileged desktop scope (the dev host; any non-Windows target).
    /// Every item is the user's own — `classify` returns `None`. This is CORRECT, not a fallback.
    Unprivileged,
    /// Resolved Windows known-folder roots. Both `Public Desktop` and `ProgramData` always exist on
    /// Windows, so both lists are guaranteed non-empty (see [`ScopeRoots::resolved`]).
    Resolved { public: Vec<String>, programdata: Vec<String> },
    /// The platform HAS a privileged scope but the roots are not yet resolved — e.g. the Windows host
    /// before `SHGetKnownFolderPath` runs, or after it FAILED. `classify` returns
    /// [`PrivilegedScope::Undetermined`] for every path, so automation FAILS CLOSED (styles nothing)
    /// rather than writing machine-wide state as if it were per-user.
    Unresolved,
}

impl ScopeRoots {
    /// Builds resolved Windows roots, FAILING CLOSED (`Err`) if either known-folder list is
    /// effectively empty. An empty list on Windows means resolution failed; the caller must fall back
    /// to [`ScopeRoots::Unresolved`], never run with it.
    pub fn resolved(
        public: Vec<String>,
        programdata: Vec<String>,
    ) -> Result<Self, EmptyScopeRoots> {
        let nonempty = |v: &[String]| v.iter().any(|r| !r.trim().is_empty());
        if !nonempty(&public) || !nonempty(&programdata) {
            return Err(EmptyScopeRoots);
        }
        Ok(Self::Resolved { public, programdata })
    }

    /// Classifies `path`'s write scope. `Unprivileged` platforms always return `None`; `Unresolved`
    /// platforms return `Some(Undetermined)` so every caller's `.is_some()` fail-closed branch fires.
    pub fn classify(&self, path: &str) -> Option<PrivilegedScope> {
        match self {
            ScopeRoots::Unprivileged => None,
            ScopeRoots::Resolved { public, programdata } => {
                privileged_scope(path, public, programdata)
            }
            ScopeRoots::Unresolved => Some(PrivilegedScope::Undetermined),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestry_is_component_wise_not_prefix() {
        let public = vec![r"C:\Users\Public\Desktop".to_string()];
        let pd = vec![r"C:\ProgramData".to_string()];
        // Real descendants match.
        assert_eq!(
            privileged_scope(r"C:\Users\Public\Desktop\Tool.lnk", &public, &pd),
            Some(PrivilegedScope::PublicDesktop)
        );
        assert_eq!(
            privileged_scope(r"C:\ProgramData\App\i.lnk", &public, &pd),
            Some(PrivilegedScope::ProgramData)
        );
        // The root itself matches.
        assert_eq!(
            privileged_scope(r"C:\Users\Public\Desktop", &public, &pd),
            Some(PrivilegedScope::PublicDesktop)
        );
        // Same-prefix SIBLINGS must NOT match (the whole point of ancestry vs starts_with).
        assert_eq!(privileged_scope(r"C:\Users\Public\DesktopBackup\x.lnk", &public, &pd), None);
        assert_eq!(privileged_scope(r"C:\ProgramDataX\x.lnk", &public, &pd), None);
        // A user's own desktop is never privileged.
        assert_eq!(privileged_scope(r"C:\Users\Dev\Desktop\mine.lnk", &public, &pd), None);
        // Case- and separator-insensitive.
        assert_eq!(
            privileged_scope("c:/users/public/desktop/tool.lnk", &public, &pd),
            Some(PrivilegedScope::PublicDesktop)
        );
        // A user desktop literally NAMED with a ProgramData-looking component is NOT matched by a
        // stray substring (the old `contains("/programdata/")` would have false-matched).
        assert_eq!(privileged_scope(r"C:\Users\Dev\ProgramData notes\x.lnk", &public, &pd), None);
    }

    #[test]
    fn raw_predicate_with_empty_roots_matches_nothing() {
        // The RAW predicate is a pure ancestry test: no roots → no ancestry → `None`. On its own
        // that is FAIL-OPEN, which is exactly why production never calls it with empty roots —
        // `ScopeRoots` (below) makes the empty-Windows-roots state unrepresentable / fail-closed.
        assert_eq!(privileged_scope(r"C:\anything", &[], &[]), None);
        assert_eq!(privileged_scope(r"C:\x", &["".to_string()], &["".to_string()]), None);
    }

    #[test]
    fn scope_roots_unresolved_fails_closed_for_every_path() {
        // The whole point: a Windows host that has NOT resolved its known folders must style
        // NOTHING through automation, not silently treat Public Desktop as the user's own.
        let unresolved = ScopeRoots::Unresolved;
        assert_eq!(unresolved.classify(r"C:\Users\Dev\Desktop\mine.lnk"), Some(PrivilegedScope::Undetermined));
        assert_eq!(unresolved.classify(r"C:\Users\Public\Desktop\tool.lnk"), Some(PrivilegedScope::Undetermined));
        // Every classify is `Some(_)`, so every caller's `.is_some()` scope-exclusion branch fires.
        assert!(unresolved.classify(r"C:\anything").is_some());
    }

    #[test]
    fn scope_roots_unprivileged_never_excludes() {
        // The dev host / non-Windows: no privileged scope exists, so nothing is excluded — correct.
        assert_eq!(ScopeRoots::Unprivileged.classify(r"C:\Users\Public\Desktop\x.lnk"), None);
    }

    #[test]
    fn scope_roots_resolved_rejects_empty_but_classifies_real_roots() {
        // A degenerate (all-empty) resolved value cannot be built — the caller must fall to Unresolved.
        assert_eq!(ScopeRoots::resolved(vec![], vec![]), Err(EmptyScopeRoots));
        assert_eq!(ScopeRoots::resolved(vec!["C:/Users/Public/Desktop".into()], vec![]), Err(EmptyScopeRoots));
        assert_eq!(ScopeRoots::resolved(vec!["  ".into()], vec!["C:/ProgramData".into()]), Err(EmptyScopeRoots));
        let roots = ScopeRoots::resolved(
            vec!["C:/Users/Public/Desktop".into()],
            vec!["C:/ProgramData".into()],
        )
        .expect("non-empty roots resolve");
        assert_eq!(roots.classify(r"C:\Users\Public\Desktop\t.lnk"), Some(PrivilegedScope::PublicDesktop));
        assert_eq!(roots.classify(r"C:\ProgramData\a\i.lnk"), Some(PrivilegedScope::ProgramData));
        assert_eq!(roots.classify(r"C:\Users\Dev\Desktop\mine.lnk"), None);
    }

    #[test]
    fn dot_dot_traversal_is_folded_before_the_gate() {
        // codex r3-🔴: `..` must be resolved so the gate sees the real destination.
        let public = vec![r"C:\Users\Public\Desktop".to_string()];
        let pd = vec![r"C:\ProgramData".to_string()];
        // Escapes the privileged dir → NOT privileged (was a false-positive under raw prefix).
        assert_eq!(privileged_scope(r"C:\Users\Public\Desktop\..\Private\x.lnk", &public, &pd), None);
        // Traverses INTO the privileged dir → IS privileged (was a false-negative).
        assert_eq!(
            privileged_scope(r"C:\Users\Public\DesktopBackup\..\Desktop\x.lnk", &public, &pd),
            Some(PrivilegedScope::PublicDesktop)
        );
        // `..` cannot escape the drive anchor (no path above C:).
        assert_eq!(privileged_scope(r"C:\..\..\Users\Public\Desktop\x", &public, &pd), Some(PrivilegedScope::PublicDesktop));
        // A `.` segment is a no-op.
        assert_eq!(
            privileged_scope(r"C:\Users\Public\Desktop\.\x.lnk", &public, &pd),
            Some(PrivilegedScope::PublicDesktop)
        );
    }

    #[test]
    fn unc_paths_are_anchored_and_compared() {
        let public = vec![r"\\srv\share\Public\Desktop".to_string()];
        assert_eq!(
            privileged_scope(r"\\srv\share\Public\Desktop\x.lnk", &public, &[]),
            Some(PrivilegedScope::PublicDesktop)
        );
        assert_eq!(privileged_scope(r"\\srv\share\Public\DesktopX\x.lnk", &public, &[]), None);
    }
}
