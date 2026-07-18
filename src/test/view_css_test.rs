use super::*;

#[test]
fn opacity_css_clamps_to_the_persisted_config_range() {
    assert!(opacity_css(f64::NEG_INFINITY).contains("0.150"));
    assert!(opacity_css(2.0).contains("1.000"));
}

#[test]
fn font_css_quotes_and_escapes_every_family() {
    let css = font_css(r#"Noto Sans, Family "Quoted", Back\Slash"#);

    assert!(css.contains(r#""Noto Sans""#));
    assert!(css.contains(r#""Family \"Quoted\"""#));
    assert!(css.contains(r#""Back\\Slash""#));
}
