// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Reusable GTK view builders for the settings frontend.

use std::{cell::RefCell, rc::Rc};

use floatlyrics_core::i18n::{I18n, Text};
use gtk::prelude::*;

use crate::frontend::localization::{bind_label, bind_stack_page_title};

use super::{ConfigChange, SettingsMsg};

pub(super) fn connect_window_i32(
    input: &gtk::SpinButton,
    sender: &relm4::Sender<SettingsMsg>,
    change: impl Fn(i32) -> ConfigChange + 'static,
) {
    let sender = sender.clone();
    input.connect_value_changed(move |input| {
        let _ = sender.send(SettingsMsg::Change(change(input.value_as_int())));
    });
}

pub(super) fn add_page(
    stack: &gtk::Stack,
    name: &str,
    title: Text,
    icon_name: &str,
    child: &impl IsA<gtk::Widget>,
    i18n: &I18n,
) {
    let page = stack.add_titled(child, Some(name), "");
    page.set_icon_name(icon_name);
    bind_stack_page_title(&page, i18n, title);
}

pub(super) fn page(
    i18n: &I18n,
    title_key: Text,
    description_key: Text,
    cards: &[gtk::Box],
) -> gtk::ScrolledWindow {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.add_css_class("settings-page");

    let title = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .css_classes(["title-1"])
        .build();
    bind_label(&title, i18n, title_key);
    let description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    bind_label(&description, i18n, description_key);
    content.append(&title);
    content.append(&description);
    for card in cards {
        content.append(card);
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build()
}

pub(super) fn setting_card(rows: &[gtk::Box]) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-card");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        card.append(row);
    }
    card
}

pub(super) fn setting_row(
    i18n: &I18n,
    title_key: Text,
    description_key: Text,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
    labels.set_hexpand(true);
    let title = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .css_classes(["settings-row-title"])
        .build();
    bind_label(&title, i18n, title_key);
    labels.append(&title);
    let description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .max_width_chars(42)
        .css_classes(["dim-label"])
        .build();
    bind_label(&description, i18n, description_key);
    labels.append(&description);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    row.add_css_class("settings-row");
    row.append(&labels);
    row.append(control);
    row
}

pub(super) fn color_row(
    i18n: &I18n,
    title_key: Text,
    description_key: Text,
    initial_hex: &str,
    sender: relm4::Sender<SettingsMsg>,
    change: fn(String) -> ConfigChange,
) -> gtk::Box {
    let current = Rc::new(RefCell::new(initial_hex.to_string()));

    let swatch = gtk::DrawingArea::builder()
        .width_request(26)
        .height_request(26)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    {
        let current = Rc::clone(&current);
        swatch.set_draw_func(move |_, cr, width, height| {
            let (r, g, b, a) = crate::shared::config::parse_hex_color(&current.borrow());
            cr.set_source_rgba(r, g, b, a);
            cr.rectangle(1.0, 1.0, (width as f64) - 2.0, (height as f64) - 2.0);
            let _ = cr.fill();
        });
    }

    let hex_label = gtk::Label::new(Some(initial_hex));
    hex_label.set_css_classes(&["dim-label", "caption"]);

    let button_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    button_content.set_halign(gtk::Align::Center);
    button_content.append(&swatch);
    button_content.append(&hex_label);

    let button = gtk::Button::builder()
        .child(&button_content)
        .width_request(190)
        .build();

    {
        let current = Rc::clone(&current);
        let swatch = swatch.clone();
        let hex_label = hex_label.clone();
        button.connect_clicked(move |btn| {
            let dialog = gtk::ColorDialog::new();
            dialog.set_with_alpha(true);
            let parent = btn.root().and_downcast::<gtk::Window>();
            let initial = hex_to_gdk_rgba(&current.borrow());
            dialog.choose_rgba(
                parent.as_ref(),
                Some(&initial),
                gtk::gio::Cancellable::NONE,
                {
                    let current = Rc::clone(&current);
                    let swatch = swatch.clone();
                    let hex_label = hex_label.clone();
                    let sender = sender.clone();
                    move |result| {
                        if let Ok(color) = result {
                            let hex = gdk_rgba_to_hex(&color);
                            current.replace(hex.clone());
                            hex_label.set_label(&hex);
                            swatch.queue_draw();
                            let _ = sender.send(SettingsMsg::Change(change(hex)));
                        }
                    }
                },
            );
        });
    }

    setting_row(i18n, title_key, description_key, &button)
}

fn hex_to_gdk_rgba(hex: &str) -> gtk::gdk::RGBA {
    let (r, g, b, a) = crate::shared::config::parse_hex_color(hex);
    gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32)
}

fn gdk_rgba_to_hex(color: &gtk::gdk::RGBA) -> String {
    crate::shared::config::format_hex_color((
        color.red() as f64,
        color.green() as f64,
        color.blue() as f64,
        color.alpha() as f64,
    ))
}

pub(super) fn install_css() {
    crate::frontend::style::install(
        r#"
        .settings-page {
            padding: 28px 36px 40px;
        }

        .settings-card {
            border: 1px solid alpha(@borders, 0.72);
            border-radius: 14px;
            background: alpha(@theme_base_color, 0.78);
            box-shadow: 0 1px 3px alpha(black, 0.08);
        }

        .settings-row {
            min-height: 48px;
            padding: 14px 16px;
        }

        .settings-row-title {
            font-weight: 650;
        }

        .settings-card separator {
            margin-left: 16px;
            background: alpha(@borders, 0.56);
        }

        .settings-status-bar {
            padding: 9px 16px;
            border-top: 1px solid alpha(@borders, 0.6);
            background: alpha(@theme_base_color, 0.45);
        }

        .settings-status.error {
            color: @error_color;
        }

        .settings-window button.suggested-action:active {
            filter: brightness(0.92);
        }
        "#,
    );
}
