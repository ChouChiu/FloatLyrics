use super::*;

#[test]
fn font_family_ignores_blank_entries_and_has_a_safe_fallback() {
    assert_eq!(font_family(&[]), "Sans");
    assert_eq!(
        font_family(&[" Noto Sans ".to_string(), "".to_string()]),
        "Noto Sans"
    );
}
