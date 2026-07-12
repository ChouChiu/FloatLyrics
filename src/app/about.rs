// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! About and acknowledgements window.

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use serde::Deserialize;

use floatlyrics_core::i18n::{I18n, Text};

use super::localization::{bind_label, bind_stack_page_title, bind_window_title};

const WINDOW_WIDTH: i32 = 560;
const WINDOW_HEIGHT: i32 = 450;
const PROJECT_URI: &str = "https://github.com/ChouChiu/FloatLyrics";
const LYRICS_X_URI: &str = "https://github.com/MxIris-LyricsX-Project/LyricsX";
const ACKNOWLEDGEMENTS_ICON: &str = "system-users-symbolic";

pub(super) struct AboutModel {
    visible: bool,
}

#[derive(Debug)]
pub(super) enum AboutMsg {
    Show,
    Hide,
}

#[relm4::component(pub(super))]
impl SimpleComponent for AboutModel {
    type Init = I18n;
    type Input = AboutMsg;
    type Output = ();

    view! {
        window = gtk::ApplicationWindow {
            set_application: Some(&relm4::main_application()),
            set_default_size: (WINDOW_WIDTH, WINDOW_HEIGHT),
            set_resizable: false,
            set_hide_on_close: true,
            add_css_class: "about-window",
            #[watch]
            set_visible: model.visible,

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                set_show_title_buttons: true,
                #[wrap(Some)]
                set_title_widget = &gtk::StackSwitcher {
                    set_stack: Some(stack),
                    set_halign: gtk::Align::Center,
                },
            },

            #[local_ref]
            stack -> gtk::Stack {},

            connect_close_request[sender] => move |_| {
                sender.input(AboutMsg::Hide);
                gtk::glib::Propagation::Proceed
            },
        }
    }

    fn init(
        i18n: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
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
        acknowledgements_stack_page.set_icon_name(ACKNOWLEDGEMENTS_ICON);
        bind_stack_page_title(&acknowledgements_stack_page, &i18n, Text::Acknowledgements);

        let dependencies_page = dependencies_page(&i18n);
        let dependencies_stack_page =
            stack.add_titled(&dependencies_page, Some("dependencies"), "");
        dependencies_stack_page.set_icon_name("application-x-sharedlib-symbolic");
        bind_stack_page_title(&dependencies_stack_page, &i18n, Text::OpenSourceTitle);

        let stack = &stack;
        let model = Self { visible: false };
        let widgets = view_output!();
        bind_window_title(&root, &i18n, Text::AboutWindowTitle);
        install_css();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        self.visible = matches!(message, AboutMsg::Show);
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
    let icon = gtk::Image::from_icon_name(ACKNOWLEDGEMENTS_ICON);
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

#[derive(Deserialize)]
struct LicenseData {
    dependencies: Vec<Dependency>,
    licenses: Vec<DependencyLicense>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    version: String,
    license: String,
}

#[derive(Deserialize)]
struct DependencyLicense {
    name: String,
    id: String,
    text: String,
}

fn dependencies_page(i18n: &I18n) -> gtk::ScrolledWindow {
    let license_data: LicenseData = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/licenses/dependencies.json"
    )))
    .expect("cargo-about generated valid dependency license data");

    let description = localized_label(i18n, Text::OpenSourceDescription, &["dim-label"]);
    description.set_wrap(true);
    description.set_max_width_chars(52);
    description.set_halign(gtk::Align::Center);

    let list = gtk::ListBox::builder()
        .css_classes(["rich-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();

    for dependency in license_data.dependencies {
        let name_label = gtk::Label::builder()
            .label(dependency.name)
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        let meta = gtk::Label::builder()
            .label(format!(
                "v{}  —  {}",
                dependency.version, dependency.license
            ))
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

    let license_title = localized_label(i18n, Text::LicenseTexts, &["title-3"]);
    license_title.set_halign(gtk::Align::Start);
    license_title.set_margin_top(12);
    content.append(&license_title);

    for license in license_data.licenses {
        let text = gtk::Label::builder()
            .label(license.text)
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .css_classes(["license-text", "caption"])
            .build();
        let expander = gtk::Expander::builder()
            .label(format!("{} ({})", license.name, license.id))
            .child(&text)
            .build();
        content.append(&expander);
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build()
}

fn install_css() {
    super::style::install(
        r#"
        .about-page { padding: 36px 48px; }
        .about-summary { font-size: 1.08em; }
        .acknowledgements-icon { color: @accent_color; }
        .license-text { padding: 12px 18px; }
        "#,
    );
}
