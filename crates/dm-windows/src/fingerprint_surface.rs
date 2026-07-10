//! Pure, host-testable derivations of an item's CAS fingerprint over the **styleable surface** —
//! the exact state an apply changes — kept separate from the `[WINDOWS-VERIFY]` code that reads
//! that state off disk/registry so the derivation logic is unit-tested on the Mac host.
//!
//! The invariant these encode: `read_fingerprint` must fingerprint *what apply mutates*, so the
//! driver's post-apply verify (P1-4) can actually confirm the styling landed. Fingerprinting a
//! surface the apply never touches makes styled and unstyled indistinguishable, and the item can
//! never commit (P1-10).

use dm_domain::Fingerprint;

/// The fingerprint of a loose file's styleable surface: the companion wrapper `.lnk` (its presence
/// and, when present, its bytes) plus the original file's attribute bits.
///
/// The file-wrapper apply styles a loose file by creating a sibling `<file>.lnk` and setting
/// `Hidden`+`System` on the original — it never rewrites the file's own bytes. Fingerprinting the
/// untouched file bytes (as an earlier revision did) therefore yields the *same* value before and
/// after styling, so the driver's verify could never confirm the apply and a `RegularFile` item
/// could never commit. This covers precisely the surface apply changes, so styled ≠ unstyled.
pub fn regular_file(wrapper_bytes: Option<&[u8]>, file_attributes: u32) -> Fingerprint {
    // A one-byte presence flag distinguishes "no wrapper" from "an empty wrapper", which the
    // length-prefixed framing of the bytes part alone would not.
    let present = [u8::from(wrapper_bytes.is_some())];
    let bytes = wrapper_bytes.unwrap_or(&[]);
    Fingerprint::of_parts(&[&present, bytes, &file_attributes.to_le_bytes()])
}

#[cfg(test)]
mod tests {
    use super::*;

    // Windows `FILE_ATTRIBUTE_*` bits used to model the styling attribute change.
    const NORMAL: u32 = 0x80;
    const HIDDEN: u32 = 0x02;
    const SYSTEM: u32 = 0x04;

    #[test]
    fn styled_surface_differs_from_unstyled_so_regularfile_can_commit() {
        // Unstyled: no wrapper, plain attributes.
        let unstyled = regular_file(None, NORMAL);
        // Styled: apply added a wrapper `.lnk` and set Hidden+System on the original.
        let styled = regular_file(Some(b"styled .lnk bytes"), NORMAL | HIDDEN | SYSTEM);
        assert_ne!(
            unstyled, styled,
            "the styled surface must fingerprint differently, else the driver's verify can never \
             confirm a RegularFile apply and the item can never commit (P1-10)"
        );

        // The regression it guards: the loose file's OWN bytes never change on apply, so a
        // fingerprint over those bytes is identical before and after — exactly the P1-10 blind
        // spot. Prove the file-bytes surface really is blind here (the bug), and that our surface
        // fingerprint is not.
        let file_bytes = b"the loose file's contents are untouched by styling";
        assert_eq!(
            Fingerprint::of_bytes(file_bytes),
            Fingerprint::of_bytes(file_bytes),
            "fingerprinting the untouched file bytes cannot distinguish styled from unstyled"
        );
    }

    #[test]
    fn empty_wrapper_differs_from_absent_wrapper() {
        // Creating an empty wrapper is still a styling change and must register.
        assert_ne!(regular_file(None, NORMAL), regular_file(Some(b""), NORMAL));
    }

    #[test]
    fn hiding_the_original_alone_changes_the_fingerprint() {
        // Even with no wrapper bytes captured yet, flipping Hidden+System must be visible.
        assert_ne!(regular_file(None, NORMAL), regular_file(None, NORMAL | HIDDEN | SYSTEM));
    }

    #[test]
    fn identical_surfaces_fingerprint_equal() {
        // Determinism: capture-time and read-back of the same styled surface must agree so CAS
        // re-apply sees an unchanged item.
        let a = regular_file(Some(b"wrapper"), NORMAL | HIDDEN | SYSTEM);
        let b = regular_file(Some(b"wrapper"), NORMAL | HIDDEN | SYSTEM);
        assert_eq!(a, b);
    }
}
