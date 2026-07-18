use super::*;

#[test]
fn font_selection_rejects_blank_and_duplicate_families() {
    let mut fonts = FontSelection::new(vec!["Sans".to_string()]);

    assert!(!fonts.add("  ".to_string()));
    assert!(!fonts.add("Sans".to_string()));
    assert!(fonts.add("Serif".to_string()));
    assert_eq!(fonts.fonts(), ["Sans", "Serif"]);
}

#[test]
fn font_selection_preserves_order_and_never_removes_the_last_family() {
    let mut fonts = FontSelection::new(vec!["Sans".to_string(), "Serif".to_string()]);

    assert!(fonts.move_by(1, -1));
    assert_eq!(fonts.fonts(), ["Serif", "Sans"]);
    assert!(fonts.remove(1));
    assert!(!fonts.remove(0));
    assert_eq!(fonts.into_fonts(), ["Serif"]);
}
