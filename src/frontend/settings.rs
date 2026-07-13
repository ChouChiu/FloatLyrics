// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Preferences frontend opened from the command line or floating panel.

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::Cell, cell::RefCell, path::PathBuf, rc::Rc};

use floatlyrics_core::i18n::{I18n, Language, Text};

use crate::shared::config::{AppConfig, ChineseRomanizationMode, LyricsProvider, WindowPosition};

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
    draft: Rc<RefCell<AppConfig>>,
    config_file: PathBuf,
    status: gtk::Label,
    status_state: Rc<RefCell<SaveState>>,
    i18n: I18n,
}

#[derive(Debug)]
pub(super) enum SettingsMsg {
    Show,
    Hide,
    OpenAbout,
    SetLanguage(Language),
    SetOffset(i64),
    SetTranslation(bool),
    SetRomanization(bool),
    SetChineseRomanization(ChineseRomanizationMode),
    SetWidth(i32),
    SetRememberPosition(bool),
    SetWindowPosition(WindowPosition),
    SetMargin(i32),
    SetPanelHeight(i32),
    SetOpacity(f64),
    SetFonts(Vec<String>),
    SetProviderOrder(Vec<LyricsProvider>),
    SetLyricFontSize(i32),
    SetTranslationFontSize(i32),
    SetRomanizationFontSize(i32),
    SetPlayedColor(String),
    SetUnplayedColor(String),
    SetTranslationColor(String),
    SetRomanizationColor(String),
}

#[derive(Debug)]
pub(super) enum SettingsOutput {
    Saved(Box<AppConfig>),
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
            "emblem-system-symbolic",
            &general_page(&initial, sender.input_sender(), &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "display",
            Text::Display,
            "video-display-symbolic",
            &display_page(&initial, &draft, sender.input_sender(), &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "sources",
            Text::LyricsSources,
            "view-list-symbolic",
            &sources_page(&initial, sender.input_sender(), &i18n),
            &i18n,
        );

        let stack = &stack;
        let status = &status;
        let model = Self {
            visible: false,
            draft,
            config_file,
            status: status.clone(),
            status_state,
            i18n: i18n.clone(),
        };
        let widgets = view_output!();
        bind_window_title(&root, &i18n, Text::SettingsWindowTitle);
        bind_button_tooltip(&widgets.about_button, &i18n, Text::OpenAbout);
        install_css();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        let should_persist = !matches!(
            message,
            SettingsMsg::Show | SettingsMsg::Hide | SettingsMsg::OpenAbout
        );
        match message {
            SettingsMsg::Show => self.visible = true,
            SettingsMsg::Hide => self.visible = false,
            SettingsMsg::OpenAbout => {
                let _ = sender.output(SettingsOutput::OpenAbout);
            }
            SettingsMsg::SetLanguage(value) => self.draft.borrow_mut().general.language = value,
            SettingsMsg::SetOffset(value) => self.draft.borrow_mut().lyrics.offset_ms = value,
            SettingsMsg::SetTranslation(value) => {
                self.draft.borrow_mut().lyrics.show_translation = value;
            }
            SettingsMsg::SetRomanization(value) => {
                self.draft.borrow_mut().lyrics.show_romanization = value;
            }
            SettingsMsg::SetChineseRomanization(value) => {
                self.draft.borrow_mut().lyrics.chinese_romanization = value;
            }
            SettingsMsg::SetWidth(value) => self.draft.borrow_mut().window.width = value,
            SettingsMsg::SetRememberPosition(value) => {
                let mut draft = self.draft.borrow_mut();
                draft.window.remember_position = value;
                if !value {
                    draft.window.position = None;
                }
            }
            SettingsMsg::SetWindowPosition(value) => {
                self.draft.borrow_mut().window.position = Some(value);
            }
            SettingsMsg::SetMargin(value) => self.draft.borrow_mut().window.margin = value,
            SettingsMsg::SetPanelHeight(value) => {
                self.draft.borrow_mut().window.bottom_panel_height = value;
            }
            SettingsMsg::SetOpacity(value) => self.draft.borrow_mut().window.opacity = value,
            SettingsMsg::SetFonts(value) => self.draft.borrow_mut().lyrics.font_order = value,
            SettingsMsg::SetProviderOrder(value) => {
                self.draft.borrow_mut().lyrics.provider_order = value;
            }
            SettingsMsg::SetLyricFontSize(value) => {
                self.draft.borrow_mut().lyrics.lyric_font_size = value;
            }
            SettingsMsg::SetTranslationFontSize(value) => {
                self.draft.borrow_mut().lyrics.translation_font_size = value;
            }
            SettingsMsg::SetRomanizationFontSize(value) => {
                self.draft.borrow_mut().lyrics.romanization_font_size = value;
            }
            SettingsMsg::SetPlayedColor(value) => {
                self.draft.borrow_mut().lyrics.played_color = value;
            }
            SettingsMsg::SetUnplayedColor(value) => {
                self.draft.borrow_mut().lyrics.unplayed_color = value;
            }
            SettingsMsg::SetTranslationColor(value) => {
                self.draft.borrow_mut().lyrics.translation_color = value;
            }
            SettingsMsg::SetRomanizationColor(value) => {
                self.draft.borrow_mut().lyrics.romanization_color = value;
            }
        }
        if should_persist {
            let next = self.draft.borrow().clone();
            match next.save(&self.config_file) {
                Ok(()) => {
                    *self.status_state.borrow_mut() = SaveState::Saved;
                    self.status
                        .set_label(&self.status_state.borrow().render(self.i18n.language()));
                    self.status.remove_css_class("error");
                    let _ = sender.output(SettingsOutput::Saved(Box::new(next)));
                }
                Err(error) => {
                    *self.status_state.borrow_mut() = SaveState::Failed(error.to_string());
                    self.status
                        .set_label(&self.status_state.borrow().render(self.i18n.language()));
                    self.status.add_css_class("error");
                }
            }
        }
    }
}

fn general_page(
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
            let _ = sender.send(SettingsMsg::SetLanguage(next_language));
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
        let sender = sender.clone();
        offset.connect_value_changed(move |input| {
            let _ = sender.send(SettingsMsg::SetOffset(input.value_as_int() as i64));
        });
    }

    let translation = gtk::Switch::builder()
        .active(config.lyrics.show_translation)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        translation.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::SetTranslation(input.is_active()));
        });
    }

    let romanization = gtk::Switch::builder()
        .active(config.lyrics.show_romanization)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        romanization.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::SetRomanization(input.is_active()));
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
            let _ = sender.send(SettingsMsg::SetChineseRomanization(mode));
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

fn display_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    sender: &relm4::Sender<SettingsMsg>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let fonts = gtk::Button::with_label("");
    fonts.set_width_request(190);
    {
        let fonts = fonts.clone();
        i18n.subscribe(move |language| fonts.set_label(language.text(Text::ChangeFonts)));
    }
    {
        let draft = Rc::clone(draft);
        let sender = sender.clone();
        let i18n = i18n.clone();
        fonts.connect_clicked(move |_| {
            let persist: Rc<dyn Fn()> = {
                let draft = Rc::clone(&draft);
                let sender = sender.clone();
                Rc::new(move || {
                    let fonts = draft.borrow().lyrics.font_order.clone();
                    let _ = sender.send(SettingsMsg::SetFonts(fonts));
                })
            };
            font_window(&draft, &persist, &i18n).present();
        });
    }

    let width = gtk::SpinButton::with_range(320.0, 640.0, 10.0);
    width.set_value(config.window.width as f64);
    width.set_numeric(true);
    width.set_width_chars(8);
    connect_window_i32(&width, sender, SettingsMsg::SetWidth);

    let remember_position = gtk::Switch::builder()
        .active(config.window.remember_position)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        remember_position.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::SetRememberPosition(input.is_active()));
        });
    }

    let margin = gtk::SpinButton::with_range(0.0, 500.0, 4.0);
    margin.set_value(config.window.margin as f64);
    margin.set_numeric(true);
    margin.set_width_chars(8);
    connect_window_i32(&margin, sender, SettingsMsg::SetMargin);

    let panel_height = gtk::SpinButton::with_range(0.0, 200.0, 2.0);
    panel_height.set_value(config.window.bottom_panel_height as f64);
    panel_height.set_numeric(true);
    panel_height.set_width_chars(8);
    connect_window_i32(&panel_height, sender, SettingsMsg::SetPanelHeight);

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.15, 1.0, 0.01);
    opacity.set_value(config.window.opacity.clamp(0.15, 1.0));
    opacity.set_draw_value(true);
    opacity.set_digits(2);
    opacity.set_width_request(200);
    {
        let sender = sender.clone();
        opacity.connect_value_changed(move |input| {
            let _ = sender.send(SettingsMsg::SetOpacity(input.value()));
        });
    }

    let lyric_font_size = gtk::SpinButton::with_range(12.0, 56.0, 1.0);
    lyric_font_size.set_value(config.lyrics.lyric_font_size as f64);
    lyric_font_size.set_numeric(true);
    lyric_font_size.set_width_chars(8);
    connect_window_i32(&lyric_font_size, sender, SettingsMsg::SetLyricFontSize);

    let translation_font_size = gtk::SpinButton::with_range(8.0, 36.0, 1.0);
    translation_font_size.set_value(config.lyrics.translation_font_size as f64);
    translation_font_size.set_numeric(true);
    translation_font_size.set_width_chars(8);
    connect_window_i32(
        &translation_font_size,
        sender,
        SettingsMsg::SetTranslationFontSize,
    );

    let romanization_font_size = gtk::SpinButton::with_range(8.0, 36.0, 1.0);
    romanization_font_size.set_value(config.lyrics.romanization_font_size as f64);
    romanization_font_size.set_numeric(true);
    romanization_font_size.set_width_chars(8);
    connect_window_i32(
        &romanization_font_size,
        sender,
        SettingsMsg::SetRomanizationFontSize,
    );

    page(
        i18n,
        Text::DisplayTitle,
        Text::DisplayDescription,
        &[
            setting_card(&[
                setting_row(i18n, Text::PanelWidth, Text::PanelWidthDescription, &width),
                setting_row(
                    i18n,
                    Text::RememberWindowPosition,
                    Text::RememberWindowPositionDescription,
                    &remember_position,
                ),
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
                setting_row(i18n, Text::Fonts, Text::FontsDescription, &fonts),
            ]),
            setting_card(&[
                setting_row(
                    i18n,
                    Text::LyricFontSize,
                    Text::LyricFontSizeDescription,
                    &lyric_font_size,
                ),
                setting_row(
                    i18n,
                    Text::TranslationFontSize,
                    Text::TranslationFontSizeDescription,
                    &translation_font_size,
                ),
                setting_row(
                    i18n,
                    Text::RomanizationFontSize,
                    Text::RomanizationFontSizeDescription,
                    &romanization_font_size,
                ),
            ]),
            setting_card(&[
                color_row(
                    i18n,
                    Text::PlayedColor,
                    Text::PlayedColorDescription,
                    &config.lyrics.played_color,
                    sender.clone(),
                    SettingsMsg::SetPlayedColor,
                ),
                color_row(
                    i18n,
                    Text::UnplayedColor,
                    Text::UnplayedColorDescription,
                    &config.lyrics.unplayed_color,
                    sender.clone(),
                    SettingsMsg::SetUnplayedColor,
                ),
                color_row(
                    i18n,
                    Text::TranslationColor,
                    Text::TranslationColorDescription,
                    &config.lyrics.translation_color,
                    sender.clone(),
                    SettingsMsg::SetTranslationColor,
                ),
                color_row(
                    i18n,
                    Text::RomanizationColor,
                    Text::RomanizationColorDescription,
                    &config.lyrics.romanization_color,
                    sender.clone(),
                    SettingsMsg::SetRomanizationColor,
                ),
            ]),
        ],
    )
}

fn font_window(
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    i18n: &I18n,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(&relm4::main_application())
        .default_width(700)
        .default_height(500)
        .resizable(false)
        .modal(true)
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
    refresh_font_order(&selected, draft, persist, i18n);
    {
        let selected = selected.clone();
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        let i18n = i18n.clone();
        available.connect_row_activated(move |_, row| {
            let Some(label) = row.child().and_downcast::<gtk::Label>() else {
                return;
            };
            let family = label.text().to_string();
            if !draft.borrow().lyrics.font_order.contains(&family) {
                draft.borrow_mut().lyrics.font_order.push(family);
                persist();
                refresh_font_order(&selected, &draft, &persist, &i18n);
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
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    i18n: &I18n,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let fonts = draft.borrow().lyrics.font_order.clone();
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
            let draft = Rc::clone(draft);
            let persist = Rc::clone(persist);
            let i18n = i18n.clone();
            button.connect_clicked(move |_| {
                let mut config = draft.borrow_mut();
                if delta == 0 {
                    if config.lyrics.font_order.len() > 1 {
                        config.lyrics.font_order.remove(index);
                    }
                } else {
                    let target = index.saturating_add_signed(delta);
                    if target < config.lyrics.font_order.len() {
                        config.lyrics.font_order.swap(index, target);
                    }
                }
                drop(config);
                persist();
                refresh_font_order(&list, &draft, &persist, &i18n);
            });
            row.append(&button);
        }
        list.append(&row);
    }
}

fn apply_font_preview(label: &gtk::Label, family: &str) {
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrString::new_family(family));
    label.set_attributes(Some(&attributes));
}

fn sources_page(
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
            let _ = sender.send(SettingsMsg::SetProviderOrder(order));
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

fn chinese_romanization_index(mode: ChineseRomanizationMode) -> u32 {
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

fn connect_window_i32(
    input: &gtk::SpinButton,
    sender: &relm4::Sender<SettingsMsg>,
    message: impl Fn(i32) -> SettingsMsg + 'static,
) {
    let sender = sender.clone();
    input.connect_value_changed(move |input| {
        let _ = sender.send(message(input.value_as_int()));
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

fn color_row(
    i18n: &I18n,
    title_key: Text,
    description_key: Text,
    initial_hex: &str,
    sender: relm4::Sender<SettingsMsg>,
    message: fn(String) -> SettingsMsg,
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
                            let _ = sender.send(message(hex));
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

fn install_css() {
    super::style::install(
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

#[cfg(test)]
#[path = "../test/settings_test.rs"]
mod tests;
