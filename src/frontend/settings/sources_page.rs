// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Lyrics-provider priority settings page.

use std::{cell::Cell, rc::Rc};

use floatlyrics_core::i18n::{I18n, Text};
use gtk::prelude::*;

use crate::shared::config::{AppConfig, LyricsProvider};

use super::view::{page, setting_card, setting_row};
use super::{ConfigChange, SettingsMsg};

pub(super) fn build(
    config: &AppConfig,
    sender: &relm4::Sender<SettingsMsg>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let source_model = gtk::StringList::new(&[
        i18n.text(Text::QqThenNetEase),
        i18n.text(Text::NetEaseThenQq),
    ]);
    let source_order = gtk::DropDown::builder()
        .model(&source_model)
        .width_request(250)
        .build();
    let netease_first = config.lyrics.provider_order.first() == Some(&LyricsProvider::NetEase);
    source_order.set_selected(u32::from(netease_first));
    let updating_model = Rc::new(Cell::new(false));
    {
        let sender = sender.clone();
        let updating_model = Rc::clone(&updating_model);
        source_order.connect_selected_notify(move |input| {
            if updating_model.get() {
                return;
            }
            let order = if input.selected() == 1 {
                vec![LyricsProvider::NetEase, LyricsProvider::QqMusic]
            } else {
                vec![LyricsProvider::QqMusic, LyricsProvider::NetEase]
            };
            let _ = sender.send(SettingsMsg::Change(ConfigChange::ProviderOrder(order)));
        });
    }
    {
        let source_model = source_model.clone();
        let source_order = source_order.clone();
        let updating_model = Rc::clone(&updating_model);
        i18n.subscribe(move |language| {
            updating_model.set(true);
            let selected = source_order.selected();
            source_model.splice(
                0,
                source_model.n_items(),
                &[
                    language.text(Text::QqThenNetEase),
                    language.text(Text::NetEaseThenQq),
                ],
            );
            source_order.set_selected(selected);
            updating_model.set(false);
        });
    }

    page(
        i18n,
        Text::SourcesTitle,
        Text::SourcesDescription,
        &[setting_card(&[setting_row(
            i18n,
            Text::SearchPriority,
            Text::SearchPriorityDescription,
            &source_order,
        )])],
    )
}
