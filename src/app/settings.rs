// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Preferences window opened from the command line or the floating panel.

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::Cell, cell::RefCell, path::PathBuf, rc::Rc};

use floatlyrics_core::i18n::{I18n, Language, Text};
use floatlyrics_lyrics::lyrics::LyricsProvider;

use crate::config::AppConfig;

use super::localization::{
    bind_button_tooltip, bind_label, bind_stack_page_title, bind_window_title,
};

const SETTINGS_WIDTH: i32 = 720;
const SETTINGS_HEIGHT: i32 = 560;

#[derive(Debug, Clone)]
enum SaveState {
    Automatic,
    Saved,
    Failed(String),
}

impl SaveState {
    fn render(&self, language: Language) -> String {
        match self {
            Self::Automatic => language.text(Text::ChangesSavedAutomatically).to_string(),
            Self::Saved => language.text(Text::Saved).to_string(),
            Self::Failed(error) => language.detail(Text::SaveFailed, error),
        }
    }
}

pub(super) struct SettingsInit {
    pub(super) initial: AppConfig,
    pub(super) config_file: PathBuf,
    pub(super) i18n: I18n,
}

pub(super) struct SettingsModel {
    visible: bool,
}

#[derive(Debug)]
pub(super) enum SettingsMsg {
    Show,
    Hide,
    OpenAbout,
}

#[derive(Debug)]
pub(super) enum SettingsOutput {
    Saved(AppConfig),
    OpenAbout,
}

#[relm4::component(pub(super))]
impl SimpleComponent for SettingsModel {
    type Init = SettingsInit;
    type Input = SettingsMsg;
    type Output = SettingsOutput;

    view! {
        window = gtk::ApplicationWindow {
            set_application: Some(&relm4::main_application()),
            set_default_size: (SETTINGS_WIDTH, SETTINGS_HEIGHT),
            set_resizable: false,
            set_hide_on_close: true,
            add_css_class: "settings-window",
            #[watch]
            set_visible: model.visible,

            #[wrap(Some)]
            set_titlebar = &gtk::WindowHandle {
                #[wrap(Some)]
                set_child = &gtk::HeaderBar {
                    set_show_title_buttons: true,
                    #[wrap(Some)]
                    set_title_widget = &gtk::StackSwitcher {
                        set_stack: Some(stack),
                        set_halign: gtk::Align::Center,
                    },
                    #[name = "about_button"]
                    pack_end = &gtk::Button {
                        set_icon_name: "help-about-symbolic",
                        set_css_classes: &["flat", "circular"],
                        connect_clicked[sender] => move |_| {
                            sender.input(SettingsMsg::OpenAbout);
                        },
                    },
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                #[local_ref]
                stack -> gtk::Stack {},
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    add_css_class: "settings-status-bar",
                    #[local_ref]
                    status -> gtk::Label {},
                },
            },

            connect_close_request[sender] => move |_| {
                sender.input(SettingsMsg::Hide);
                gtk::glib::Propagation::Proceed
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let SettingsInit {
            initial,
            config_file,
            i18n,
        } = init;
        let draft = Rc::new(RefCell::new(initial.clone()));
        let status_state = Rc::new(RefCell::new(SaveState::Automatic));
        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["settings-status", "dim-label"])
            .build();
        {
            let status = status.clone();
            let status_state = Rc::clone(&status_state);
            i18n.subscribe(move |language| {
                status.set_label(&status_state.borrow().render(language));
                if matches!(*status_state.borrow(), SaveState::Failed(_)) {
                    status.add_css_class("error");
                } else {
                    status.remove_css_class("error");
                }
            });
        }

        let persist: Rc<dyn Fn()> = {
            let draft = Rc::clone(&draft);
            let i18n = i18n.clone();
            let status = status.clone();
            let status_state = Rc::clone(&status_state);
            let output = sender.output_sender().clone();
            Rc::new(move || {
                let next = draft.borrow().clone();
                match next.save(&config_file) {
                    Ok(()) => {
                        *status_state.borrow_mut() = SaveState::Saved;
                        status.set_label(&status_state.borrow().render(i18n.language()));
                        status.remove_css_class("error");
                        let _ = output.send(SettingsOutput::Saved(next));
                    }
                    Err(error) => {
                        *status_state.borrow_mut() = SaveState::Failed(error.to_string());
                        status.set_label(&status_state.borrow().render(i18n.language()));
                        status.add_css_class("error");
                    }
                }
            })
        };

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(180)
            .hexpand(true)
            .vexpand(true)
            .build();

        add_page(
            &stack,
            "general",
            Text::General,
            "preferences-system-symbolic",
            &general_page(&initial, &draft, &persist, &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "display",
            Text::Display,
            "video-display-symbolic",
            &display_page(&initial, &draft, &persist, &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "sources",
            Text::LyricsSources,
            "view-list-symbolic",
            &sources_page(&initial, &draft, &persist, &i18n),
            &i18n,
        );

        let stack = &stack;
        let status = &status;
        let model = Self { visible: false };
        let widgets = view_output!();
        bind_window_title(&root, &i18n, Text::SettingsWindowTitle);
        bind_button_tooltip(&widgets.about_button, &i18n, Text::OpenAbout);
        install_css();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SettingsMsg::Show => self.visible = true,
            SettingsMsg::Hide => self.visible = false,
            SettingsMsg::OpenAbout => {
                let _ = sender.output(SettingsOutput::OpenAbout);
            }
        }
    }
}

fn general_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let language_names = Language::ALL.map(Language::display_name);
    let language = gtk::DropDown::from_strings(&language_names);
    language.set_selected(language_index(config.general.language));
    language.set_width_request(190);
    let changing_language = Rc::new(Cell::new(false));
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        let changing_language = Rc::clone(&changing_language);
        language.connect_selected_notify(move |input| {
            if changing_language.get() {
                return;
            }
            let Some(next_language) = Language::ALL.get(input.selected() as usize).copied() else {
                return;
            };
            draft.borrow_mut().general.language = next_language;
            persist();
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

    let offset = gtk::SpinButton::with_range(-10_000.0, 10_000.0, 50.0);
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
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        offset.connect_value_changed(move |input| {
            draft.borrow_mut().lyrics.offset_ms = input.value_as_int() as i64;
            persist();
        });
    }

    let translation = gtk::Switch::builder()
        .active(config.lyrics.show_translation)
        .valign(gtk::Align::Center)
        .build();
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        translation.connect_active_notify(move |input| {
            draft.borrow_mut().lyrics.show_translation = input.is_active();
            persist();
        });
    }

    let romanization = gtk::Switch::builder()
        .active(config.lyrics.show_romanization)
        .valign(gtk::Align::Center)
        .build();
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        romanization.connect_active_notify(move |input| {
            draft.borrow_mut().lyrics.show_romanization = input.is_active();
            persist();
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
            ]),
        ],
    )
}

fn display_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let width = gtk::SpinButton::with_range(320.0, 640.0, 10.0);
    width.set_value(config.window.width as f64);
    width.set_numeric(true);
    width.set_width_chars(8);
    connect_window_i32(&width, draft, persist, |config, value| {
        config.window.width = value;
    });

    let margin = gtk::SpinButton::with_range(0.0, 500.0, 4.0);
    margin.set_value(config.window.margin as f64);
    margin.set_numeric(true);
    margin.set_width_chars(8);
    connect_window_i32(&margin, draft, persist, |config, value| {
        config.window.margin = value;
    });

    let panel_height = gtk::SpinButton::with_range(0.0, 200.0, 2.0);
    panel_height.set_value(config.window.bottom_panel_height as f64);
    panel_height.set_numeric(true);
    panel_height.set_width_chars(8);
    connect_window_i32(&panel_height, draft, persist, |config, value| {
        config.window.bottom_panel_height = value;
    });

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.15, 1.0, 0.01);
    opacity.set_value(config.window.opacity.clamp(0.15, 1.0));
    opacity.set_draw_value(true);
    opacity.set_digits(2);
    opacity.set_width_request(200);
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        opacity.connect_value_changed(move |input| {
            draft.borrow_mut().window.opacity = input.value();
            persist();
        });
    }

    page(
        i18n,
        Text::DisplayTitle,
        Text::DisplayDescription,
        &[setting_card(&[
            setting_row(i18n, Text::PanelWidth, Text::PanelWidthDescription, &width),
            setting_row(
                i18n,
                Text::BottomMargin,
                Text::BottomMarginDescription,
                &margin,
            ),
            setting_row(
                i18n,
                Text::BottomPanelHeight,
                Text::BottomPanelHeightDescription,
                &panel_height,
            ),
            setting_row(
                i18n,
                Text::BackgroundOpacity,
                Text::BackgroundOpacityDescription,
                &opacity,
            ),
        ])],
    )
}

fn sources_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
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
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        let updating_model = Rc::clone(&updating_model);
        source_order.connect_selected_notify(move |input| {
            if updating_model.get() {
                return;
            }
            draft.borrow_mut().lyrics.provider_order = if input.selected() == 1 {
                vec![LyricsProvider::NetEase, LyricsProvider::QqMusic]
            } else {
                vec![LyricsProvider::QqMusic, LyricsProvider::NetEase]
            };
            persist();
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

fn language_index(language: Language) -> u32 {
    Language::ALL
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or_default() as u32
}

fn connect_window_i32(
    input: &gtk::SpinButton,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    update: impl Fn(&mut AppConfig, i32) + 'static,
) {
    let draft = Rc::clone(draft);
    let persist = Rc::clone(persist);
    input.connect_value_changed(move |input| {
        update(&mut draft.borrow_mut(), input.value_as_int());
        persist();
    });
}

fn add_page(
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

fn page(
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

fn setting_card(rows: &[gtk::Box]) -> gtk::Box {
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

fn setting_row(
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

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
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
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
#[path = "../test/settings_test.rs"]
mod tests;
