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
}

/// Normalizes a path for case-insensitive component comparison: lowercase (NTFS is
/// case-insensitive) + forward slashes + no trailing separator.
fn normalize(path: &str) -> String {
    let lower = path.to_ascii_lowercase().replace('\\', "/");
    lower.trim_end_matches('/').to_string()
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
    fn empty_roots_never_match() {
        assert_eq!(privileged_scope(r"C:\anything", &[], &[]), None);
        assert_eq!(privileged_scope(r"C:\x", &["".to_string()], &["".to_string()]), None);
    }
}
