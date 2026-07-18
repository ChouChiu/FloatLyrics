// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Font discovery, preview, and ordering UI for settings.

mod state;

use std::rc::Rc;

use floatlyrics_core::i18n::{I18n, Text};
use gtk::prelude::*;

use crate::frontend::localization::{bind_button_tooltip, bind_label, bind_window_title};

use state::FontSelection;

pub(super) fn font_window(
    initial_fonts: Vec<String>,
    persist: &Rc<dyn Fn(Vec<String>)>,
    i18n: &I18n,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(&relm4::main_application())
        .default_width(700)
        .default_height(500)
        .resizable(false)
        .modal(true)
        .hide_on_close(true)
        .build();
    bind_window_title(&window, i18n, Text::FontWindowTitle);
    window.set_titlebar(Some(
        &gtk::HeaderBar::builder().show_title_buttons(true).build(),
    ));

    let available = gtk::ListBox::new();
    available.set_selection_mode(gtk::SelectionMode::None);
    available.add_css_class("boxed-list");
    let mut families = window
        .pango_context()
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<Vec<_>>();
    families.sort_by_key(|family| family.to_lowercase());
    families.dedup();
    for family in families {
        let label = gtk::Label::builder()
            .label(&family)
            .halign(gtk::Align::Start)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        apply_font_preview(&label, &family);
        available.append(&label);
    }

    let selected = gtk::ListBox::new();
    selected.set_selection_mode(gtk::SelectionMode::None);
    selected.add_css_class("boxed-list");
    refresh_font_order(&selected, &initial_fonts, persist, i18n);
    {
        let selected = selected.clone();
        let persist = Rc::clone(persist);
        let i18n = i18n.clone();
        available.connect_row_activated(move |_, row| {
            let Some(label) = row.child().and_downcast::<gtk::Label>() else {
                return;
            };
            let family = label.text().to_string();
            let mut selection = FontSelection::new(selected_font_families(&selected));
            if selection.add(family) {
                let next = selection.into_fonts();
                persist(next.clone());
                refresh_font_order(&selected, &next, &persist, &i18n);
            }
        });
    }

    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    columns.set_margin_top(18);
    columns.set_margin_bottom(18);
    columns.set_margin_start(18);
    columns.set_margin_end(18);
    columns.append(&font_column(i18n, Text::AvailableFonts, &available));
    columns.append(&font_column(i18n, Text::FontOrder, &selected));

    let done = gtk::Button::builder()
        .halign(gtk::Align::End)
        .css_classes(["suggested-action"])
        .build();
    {
        let done = done.clone();
        i18n.subscribe(move |language| done.set_label(language.text(Text::Done)));
    }
    {
        let window = window.clone();
        done.connect_clicked(move |_| window.close());
    }
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    footer.set_margin_bottom(12);
    footer.set_margin_start(18);
    footer.set_margin_end(18);
    footer.set_halign(gtk::Align::End);
    footer.append(&done);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&columns);
    content.append(&footer);
    window.set_child(Some(&content));
    window
}

fn font_column(i18n: &I18n, title_key: Text, list: &gtk::ListBox) -> gtk::Box {
    let title = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .css_classes(["heading"])
        .build();
    bind_label(&title, i18n, title_key);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(list)
        .hexpand(true)
        .vexpand(true)
        .build();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_hexpand(true);
    column.append(&title);
    column.append(&scroll);
    column
}

fn refresh_font_order(
    list: &gtk::ListBox,
    fonts: &[String],
    persist: &Rc<dyn Fn(Vec<String>)>,
    i18n: &I18n,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for (index, family) in fonts.iter().enumerate() {
        let label = gtk::Label::builder()
            .label(family)
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();
        apply_font_preview(&label, family);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(10);
        row.set_margin_end(6);
        row.append(&label);
        for (icon, delta, key) in [
            ("go-up-symbolic", -1_isize, Text::MoveFontUp),
            ("go-down-symbolic", 1_isize, Text::MoveFontDown),
            ("list-remove-symbolic", 0_isize, Text::RemoveFont),
        ] {
            let button = gtk::Button::builder()
                .icon_name(icon)
                .css_classes(["flat", "circular"])
                .build();
            bind_button_tooltip(&button, i18n, key);
            button.set_sensitive(if delta < 0 {
                index > 0
            } else if delta > 0 {
                index + 1 < fonts.len()
            } else {
                fonts.len() > 1
            });
            let list = list.clone();
            let persist = Rc::clone(persist);
            let i18n = i18n.clone();
            button.connect_clicked(move |_| {
                let mut selection = FontSelection::new(selected_font_families(&list));
                let changed = if delta == 0 {
                    selection.remove(index)
                } else {
                    selection.move_by(index, delta)
                };
                if changed {
                    let next = selection.into_fonts();
                    persist(next.clone());
                    refresh_font_order(&list, &next, &persist, &i18n);
                }
            });
            row.append(&button);
        }
        list.append(&row);
    }
}

fn selected_font_families(list: &gtk::ListBox) -> Vec<String> {
    let mut fonts = Vec::new();
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        let Some(content) = row.child().and_downcast::<gtk::Box>() else {
            index += 1;
            continue;
        };
        if let Some(label) = content.first_child().and_downcast::<gtk::Label>() {
            fonts.push(label.text().to_string());
        }
        index += 1;
    }
    fonts
}

fn apply_font_preview(label: &gtk::Label, family: &str) {
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrString::new_family(family));
    label.set_attributes(Some(&attributes));
}
