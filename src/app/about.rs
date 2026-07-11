// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! About and acknowledgements window.

use gtk::prelude::*;

use crate::i18n::{I18n, Text};

use super::localization::{bind_label, bind_stack_page_title, bind_window_title};

const WINDOW_WIDTH: i32 = 560;
const WINDOW_HEIGHT: i32 = 450;
const PROJECT_URI: &str = "https://github.com/ChouChiu/FloatLyrics";
const LYRICS_X_URI: &str = "https://github.com/MxIris-LyricsX-Project/LyricsX";

#[derive(Clone)]
pub(super) struct AboutWindow {
    window: gtk::ApplicationWindow,
}

impl AboutWindow {
    pub(super) fn new(app: &gtk::Application, i18n: I18n) -> Self {
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(180)
            .hexpand(true)
            .vexpand(true)
            .build();

        let about_page = about_page(&i18n);
        let about_stack_page = stack.add_titled(&about_page, Some("about"), "");
        about_stack_page.set_icon_name("help-about-symbolic");
        bind_stack_page_title(&about_stack_page, &i18n, Text::About);

        let acknowledgements_page = acknowledgements_page(&i18n);
        let acknowledgements_stack_page =
            stack.add_titled(&acknowledgements_page, Some("acknowledgements"), "");
        acknowledgements_stack_page.set_icon_name("emblem-favorite-symbolic");
        bind_stack_page_title(&acknowledgements_stack_page, &i18n, Text::Acknowledgements);

        let dependencies_page = dependencies_page(&i18n);
        let dependencies_stack_page =
            stack.add_titled(&dependencies_page, Some("dependencies"), "");
        dependencies_stack_page.set_icon_name("application-x-sharedlib-symbolic");
        bind_stack_page_title(&dependencies_stack_page, &i18n, Text::OpenSourceTitle);

        let switcher = gtk::StackSwitcher::builder()
            .stack(&stack)
            .halign(gtk::Align::Center)
            .build();
        let header = gtk::HeaderBar::builder()
            .title_widget(&switcher)
            .show_title_buttons(true)
            .build();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .default_width(WINDOW_WIDTH)
            .default_height(WINDOW_HEIGHT)
            .resizable(false)
            .titlebar(&header)
            .child(&stack)
            .hide_on_close(true)
            .build();
        window.add_css_class("about-window");
        bind_window_title(&window, &i18n, Text::AboutWindowTitle);
        install_css();

        Self { window }
    }

    pub(super) fn present(&self) {
        self.window.present();
    }
}

fn about_page(i18n: &I18n) -> gtk::Box {
    let icon = gtk::Image::from_icon_name("io.github.chouchiu.floatlyrics");
    icon.set_pixel_size(88);

    let name = gtk::Label::builder()
        .label("FloatLyrics")
        .css_classes(["title-1"])
        .build();
    let summary = localized_label(i18n, Text::AppSummary, &["about-summary", "dim-label"]);
    summary.set_wrap(true);
    summary.set_justify(gtk::Justification::Center);
    summary.set_max_width_chars(52);

    let version = gtk::Label::builder()
        .label(format!(
            "{} {}",
            i18n.text(Text::Version),
            env!("CARGO_PKG_VERSION")
        ))
        .css_classes(["dim-label"])
        .build();
    {
        let version = version.clone();
        i18n.subscribe(move |language| {
            version.set_label(&format!(
                "{} {}",
                language.text(Text::Version),
                env!("CARGO_PKG_VERSION")
            ));
        });
    }
    let copyright = localized_label(i18n, Text::Copyright, &[]);
    let license = localized_label(i18n, Text::LicenseStatement, &["dim-label"]);
    license.set_wrap(true);
    license.set_justify(gtk::Justification::Center);

    let website = gtk::LinkButton::with_label(PROJECT_URI, i18n.text(Text::ProjectWebsite));
    {
        let website = website.clone();
        i18n.subscribe(move |language| website.set_label(language.text(Text::ProjectWebsite)));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.add_css_class("about-page");
    content.append(&icon);
    content.append(&name);
    content.append(&summary);
    content.append(&version);
    content.append(&copyright);
    content.append(&license);
    content.append(&website);
    content
}

fn acknowledgements_page(i18n: &I18n) -> gtk::Box {
    let icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("acknowledgements-icon");
    let title = localized_label(i18n, Text::AcknowledgementsTitle, &["title-1"]);
    let description = localized_label(i18n, Text::InspiredByLyricsX, &["dim-label"]);
    description.set_wrap(true);
    description.set_justify(gtk::Justification::Center);
    description.set_max_width_chars(52);

    let lyrics_x = gtk::LinkButton::with_label(LYRICS_X_URI, i18n.text(Text::VisitLyricsX));
    {
        let lyrics_x = lyrics_x.clone();
        i18n.subscribe(move |language| lyrics_x.set_label(language.text(Text::VisitLyricsX)));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.add_css_class("about-page");
    content.append(&icon);
    content.append(&title);
    content.append(&description);
    content.append(&lyrics_x);
    content
}

fn localized_label(i18n: &I18n, key: Text, classes: &[&str]) -> gtk::Label {
    let label = gtk::Label::builder().css_classes(classes).build();
    bind_label(&label, i18n, key);
    label
}

include!(concat!(env!("OUT_DIR"), "/dep_list.rs"));

fn dependencies_page(i18n: &I18n) -> gtk::ScrolledWindow {
    let description = localized_label(i18n, Text::OpenSourceDescription, &["dim-label"]);
    description.set_wrap(true);
    description.set_max_width_chars(52);
    description.set_halign(gtk::Align::Center);

    let list = gtk::ListBox::builder()
        .css_classes(["rich-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();

    for (name, version, license) in DEPENDENCIES {
        let name_label = gtk::Label::builder()
            .label(*name)
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        let meta = gtk::Label::builder()
            .label(format!("v{version}  —  {license}"))
            .halign(gtk::Align::Start)
            .css_classes(["dim-label", "caption"])
            .build();
        let row_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row_box.set_margin_top(6);
        row_box.set_margin_bottom(6);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.append(&name_label);
        row_box.append(&meta);
        list.append(&row_box);
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("about-page");
    content.append(&description);
    content.append(&list);

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build()
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .about-page { padding: 36px 48px; }
        .about-summary { font-size: 1.08em; }
        .acknowledgements-icon { color: @accent_color; }
        "#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
