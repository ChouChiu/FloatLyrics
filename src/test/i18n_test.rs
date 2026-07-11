use super::*;

#[test]
fn detects_supported_chinese_locale_variants() {
    assert_eq!(
        Language::from_locale("zh_CN.UTF-8"),
        Language::SimplifiedChinese
    );
    assert_eq!(
        Language::from_locale("zh-Hant-HK"),
        Language::TraditionalChinese
    );
    assert_eq!(Language::from_locale("en_GB.UTF-8"), Language::English);
}

#[test]
fn language_codes_round_trip_through_toml() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct LanguageSetting {
        language: Language,
    }

    for language in Language::ALL {
        let setting = LanguageSetting { language };
        let encoded = toml::to_string(&setting).unwrap();
        let decoded: LanguageSetting = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, setting, "{} did not round trip", language.code());
    }
}

#[test]
fn changing_language_notifies_subscribers_once() {
    let i18n = I18n::new(Language::English);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_for_listener = Rc::clone(&observed);
    i18n.subscribe(move |language| observed_for_listener.borrow_mut().push(language));

    i18n.set_language(Language::SimplifiedChinese);
    i18n.set_language(Language::SimplifiedChinese);

    assert_eq!(
        observed.borrow().as_slice(),
        &[Language::English, Language::SimplifiedChinese]
    );
}
