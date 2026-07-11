use super::*;

fn line_at(text: &str, start_ms: u64) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms: None,
        text: text.to_string(),
        syllables: vec![],
        translation: None,
        romanization: None,
        background: None,
    }
}

#[test]
fn filters_netease_credit_lines() {
    assert!(is_credit_line(
        &line_at(" 演唱 Leana Mask", 6273),
        " 演唱 Leana Mask"
    ));
    assert!(is_credit_line(
        &line_at("作曲 : Marina Diamandis/Rick Nowels", 0),
        "作曲 : Marina Diamandis/Rick Nowels"
    ));
    assert!(is_credit_line(
        &line_at("作词 Marina Diamandis/Rick Nowells", 0),
        "作词 Marina Diamandis/Rick Nowells"
    ));
}

#[test]
fn filters_english_credit_lines() {
    assert!(is_credit_line(
        &line_at("Lyrics by John Doe", 0),
        "Lyrics by John Doe"
    ));
    assert!(is_credit_line(
        &line_at("Composed by Jane", 0),
        "Composed by Jane"
    ));
    assert!(is_credit_line(
        &line_at("Mixing: Engineer", 2000),
        "Mixing: Engineer"
    ));
}

#[test]
fn filters_generic_key_value_metadata_in_intro() {
    assert!(is_credit_line(
        &line_at("出品：某唱片公司", 1500),
        "出品：某唱片公司"
    ));
    assert!(is_credit_line(&line_at("配唱：某人", 300), "配唱：某人"));
}

#[test]
fn does_not_filter_real_lyrics_with_colon() {
    let line = line_at("love: it's real", 30000);
    assert!(!is_credit_line(&line, "love: it's real"));
    let line = line_at("I: am here", 5000);
    assert!(!is_credit_line(&line, "I: am here"));
}

#[test]
fn filters_intro_title_line() {
    let line = line_at("Hello World - Adele", 100);
    assert!(is_intro_title_line(&line, "Hello World - Adele"));
    let line = line_at("Hello World - Adele", 6000);
    assert!(!is_intro_title_line(&line, "Hello World - Adele"));
}

#[test]
fn filters_cjk_speaker_label() {
    assert!(is_speaker_label_line("周杰伦："));
    assert!(is_speaker_label_line("阿信："));
    assert!(!is_speaker_label_line("我们一起学猫叫"));
}

#[test]
fn filters_urls_and_typographic_marks() {
    assert!(is_credit_line(
        &line_at("http://example.com", 0),
        "http://example.com"
    ));
    assert!(is_credit_line(
        &line_at("https://example.com", 0),
        "https://example.com"
    ));
    assert!(is_credit_line(
        &line_at("www.example.com", 0),
        "www.example.com"
    ));
    assert!(is_credit_line(&line_at("℗ 2024 Label", 0), "℗ 2024 Label"));
}
