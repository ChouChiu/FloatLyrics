// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Language, timing, and secondary-text settings page.

use std::{cell::Cell, rc::Rc};

use floatlyrics_core::i18n::{I18n, Language, Text};
use gtk::prelude::*;

use crate::shared::config::{AppConfig, ChineseRomanizationMode, ConfigLimits};

use super::view::{page, setting_card, setting_row};
use super::{ConfigChange, SettingsMsg};

pub(super) fn build(
    config: &AppConfig,
    sender: &relm4::Sender<SettingsMsg>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let language_names = Language::ALL.map(Language::display_name);
    let language = gtk::DropDown::from_strings(&language_names);
    language.set_selected(language_index(config.general.language));
    language.set_width_request(190);
    let changing_language = Rc::new(Cell::new(false));
    {
        let sender = sender.clone();
        let changing_language = Rc::clone(&changing_language);
        language.connect_selected_notify(move |input| {
            if changing_language.get() {
                return;
            }
            let Some(next_language) = Language::ALL.get(input.selected() as usize).copied() else {
                return;
            };
            let _ = sender.send(SettingsMsg::Change(ConfigChange::Language(next_language)));
        });
    }
    {
        let language = language.clone();
        let changing_language = Rc::clone(&changing_language);
        i18n.subscribe(move |next_language| {
            changing_language.set(true);
            language.set_selected(language_index(next_language));
            changing_language.set(false);
        });
    }

    let offset = gtk::SpinButton::with_range(
        ConfigLimits::OFFSET_MS_MIN as f64,
        ConfigLimits::OFFSET_MS_MAX as f64,
        50.0,
    );
    offset.set_value(config.lyrics.offset_ms as f64);
    offset.set_numeric(true);
    offset.set_width_chars(8);
    {
        let offset = offset.clone();
        i18n.subscribe(move |language| {
            offset.set_tooltip_text(Some(language.text(Text::GlobalOffsetDescription)));
        });
    }
    {
        let sender = sender.clone();
        offset.connect_value_changed(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::Offset(
                input.value_as_int() as i64,
            )));
        });
    }

    let translation = gtk::Switch::builder()
        .active(config.lyrics.show_translation)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        translation.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::Translation(
                input.is_active(),
            )));
        });
    }

    let romanization = gtk::Switch::builder()
        .active(config.lyrics.show_romanization)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        romanization.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::Romanization(
                input.is_active(),
            )));
        });
    }

    let romanization_mode_model = gtk::StringList::new(&romanization_mode_names(i18n.language()));
    let romanization_mode = gtk::DropDown::builder()
        .model(&romanization_mode_model)
        .width_request(190)
        .build();
    romanization_mode.set_selected(chinese_romanization_index(
        config.lyrics.chinese_romanization,
    ));
    let updating_romanization_mode = Rc::new(Cell::new(false));
    {
        let sender = sender.clone();
        let updating = Rc::clone(&updating_romanization_mode);
        romanization_mode.connect_selected_notify(move |input| {
            if updating.get() {
                return;
            }
            let Some(mode) = ChineseRomanizationMode::ALL
                .get(input.selected() as usize)
                .copied()
            else {
                return;
            };
            let _ = sender.send(SettingsMsg::Change(ConfigChange::ChineseRomanization(mode)));
        });
    }
    {
        let model = romanization_mode_model.clone();
        let input = romanization_mode.clone();
        let updating = Rc::clone(&updating_romanization_mode);
        i18n.subscribe(move |language| {
            updating.set(true);
            let selected = input.selected();
            model.splice(0, model.n_items(), &romanization_mode_names(language));
            input.set_selected(selected);
            updating.set(false);
        });
    }

    page(
        i18n,
        Text::GeneralTitle,
        Text::GeneralDescription,
        &[
            setting_card(&[setting_row(
                i18n,
                Text::Language,
                Text::LanguageDescription,
                &language,
            )]),
            setting_card(&[
                setting_row(
                    i18n,
                    Text::GlobalOffset,
                    Text::GlobalOffsetDescription,
                    &offset,
                ),
                setting_row(
                    i18n,
                    Text::ShowTranslation,
                    Text::ShowTranslationDescription,
                    &translation,
                ),
                setting_row(
                    i18n,
                    Text::ShowRomanization,
                    Text::ShowRomanizationDescription,
                    &romanization,
                ),
                setting_row(
                    i18n,
                    Text::ChineseRomanization,
                    Text::ChineseRomanizationDescription,
                    &romanization_mode,
                ),
            ]),
        ],
    )
}

pub(super) fn language_index(language: Language) -> u32 {
    Language::ALL
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or_default() as u32
}

pub(super) fn chinese_romanization_index(mode: ChineseRomanizationMode) -> u32 {
    ChineseRomanizationMode::ALL
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or_default() as u32
}

fn romanization_mode_names(language: Language) -> [&'static str; 4] {
    [
        language.text(Text::RomanizationAutomatic),
        language.text(Text::MandarinPinyin),
        language.text(Text::CantoneseJyutping),
        language.text(Text::CantoneseJyutpingWithoutTones),
    ]
}
