// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Small frontend bindings for the runtime translation catalogue.

use gtk::prelude::*;

use floatlyrics_core::i18n::{I18n, Text};

pub(super) fn bind_label(label: &gtk::Label, i18n: &I18n, key: Text) {
    let label = label.clone();
    i18n.subscribe(move |language| label.set_label(language.text(key)));
}

pub(super) fn bind_button_label(button: &gtk::Button, i18n: &I18n, key: Text) {
    let button = button.clone();
    i18n.subscribe(move |language| button.set_label(language.text(key)));
}

pub(super) fn bind_button_tooltip(button: &gtk::Button, i18n: &I18n, key: Text) {
    let button = button.clone();
    i18n.subscribe(move |language| button.set_tooltip_text(Some(language.text(key))));
}

pub(super) fn bind_entry_placeholder(entry: &gtk::Entry, i18n: &I18n, key: Text) {
    let entry = entry.clone();
    i18n.subscribe(move |language| entry.set_placeholder_text(Some(language.text(key))));
}

pub(super) fn bind_window_title(window: &gtk::ApplicationWindow, i18n: &I18n, key: Text) {
    let window = window.clone();
    i18n.subscribe(move |language| window.set_title(Some(language.text(key))));
}

pub(super) fn bind_stack_page_title(page: &gtk::StackPage, i18n: &I18n, key: Text) {
    let page = page.clone();
    i18n.subscribe(move |language| page.set_title(language.text(key)));
}
