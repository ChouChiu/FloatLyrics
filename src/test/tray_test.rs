// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

use crate::shared::config::{AppConfig, TrayConfig};

/// Builds a tray that renders its labels in `language`.
fn tray_for(language: Language) -> FloatLyricsTray {
    FloatLyricsTray::new(Arc::new(RwLock::new(language)))
}

/// Returns the label of a standard menu entry.
fn label(item: &MenuItem<FloatLyricsTray>) -> &str {
    match item {
        MenuItem::Standard(item) => &item.label,
        MenuItem::Separator => panic!("expected a labelled menu entry, found a separator"),
        _ => panic!("expected a standard menu entry"),
    }
}

#[test]
fn menu_lists_the_application_actions_only() {
    for language in Language::ALL {
        let tray = tray_for(language);
        let menu = tray.menu();
        let context = language.code();

        assert_eq!(menu.len(), 5, "{context}");
        assert_eq!(
            label(&menu[0]),
            language.text(Text::OpenSettings),
            "{context}"
        );
        assert_eq!(
            label(&menu[1]),
            language.text(Text::SearchLyrics),
            "{context}"
        );
        assert_eq!(
            label(&menu[2]),
            language.text(Text::ReloadLyrics),
            "{context}"
        );
        assert!(matches!(menu[3], MenuItem::Separator), "{context}");
        assert_eq!(label(&menu[4]), language.text(Text::Quit), "{context}");
    }
}

#[test]
fn identity_matches_the_installed_application_icon() {
    let tray = tray_for(Language::English);

    assert_eq!(tray.id(), "io.github.chouchiu.floatlyrics");
    assert_eq!(tray.title(), "FloatLyrics");
    assert_eq!(tray.icon_name(), "io.github.chouchiu.floatlyrics");
    assert_eq!(tray.tool_tip().title, "FloatLyrics");
}

#[test]
fn spawn_returns_none_when_the_tray_is_disabled() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("the test runtime must build");
    let config = AppConfig {
        tray: TrayConfig { enabled: false },
        ..AppConfig::default()
    };

    assert!(spawn(&config, runtime.handle()).is_none());
}

#[test]
fn menu_labels_track_the_shared_language() {
    let language = Arc::new(RwLock::new(Language::English));
    let tray = FloatLyricsTray::new(Arc::clone(&language));
    assert_eq!(
        label(&tray.menu()[0]),
        Language::English.text(Text::OpenSettings)
    );

    *language
        .write()
        .expect("the tray language lock is never poisoned") = Language::SimplifiedChinese;

    assert_ne!(
        Language::English.text(Text::OpenSettings),
        Language::SimplifiedChinese.text(Text::OpenSettings)
    );
    assert_eq!(
        label(&tray.menu()[0]),
        Language::SimplifiedChinese.text(Text::OpenSettings)
    );
}
