use super::*;

#[test]
fn runtime_json_catalogues_are_complete() {
    let locale_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/locale");

    for language in Language::ALL {
        let catalog = Catalog::load_file(&locale_dir, language)
            .unwrap_or_else(|| panic!("{} catalogue is missing or incomplete", language.code()));
        assert_ne!(
            catalog.text(Text::SettingsWindowTitle),
            "SettingsWindowTitle"
        );
    }
}
