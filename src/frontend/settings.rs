// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Preferences frontend opened from the command line or floating panel.

mod display_page;
mod font_picker;
mod general_page;
mod persistence;
mod sources_page;
mod state;
mod view;

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use floatlyrics_core::i18n::{I18n, Language, Text};

use crate::shared::config::AppConfig;

use super::localization::{bind_button_tooltip, bind_window_title};
use persistence::ConfigSaveResult;
pub(super) use persistence::ConfigSaveService;
pub(super) use state::ConfigChange;
use state::SaveTracker;
use view::{add_page, install_css};

const SETTINGS_WIDTH: i32 = 720;
const SETTINGS_HEIGHT: i32 = 560;

pub(super) struct SettingsInit {
    pub(super) initial: AppConfig,
    pub(super) config_saver: ConfigSaveService,
    pub(super) i18n: I18n,
}

pub(super) struct SettingsModel {
    visible: bool,
    draft: AppConfig,
    config_saver: ConfigSaveService,
    status: gtk::Label,
    save_tracker: SaveTracker,
    language: Language,
}

#[derive(Debug)]
pub(super) enum SettingsMsg {
    Show,
    Hide,
    OpenAbout,
    Change(ConfigChange),
    SaveFinished {
        revision: u64,
        config: Box<AppConfig>,
        result: ConfigSaveResult,
    },
    LanguageChanged(Language),
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
            config_saver,
            i18n,
        } = init;
        let draft = initial.clone();
        let language = i18n.language();
        let save_tracker = SaveTracker::default();
        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["settings-status", "dim-label"])
            .build();
        status.set_label(&save_tracker.render(language));
        {
            let input = sender.input_sender().clone();
            i18n.subscribe(move |language| {
                let _ = input.send(SettingsMsg::LanguageChanged(language));
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
            &general_page::build(&initial, sender.input_sender(), &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "display",
            Text::Display,
            "video-display-symbolic",
            &display_page::build(&initial, sender.input_sender(), &i18n),
            &i18n,
        );
        add_page(
            &stack,
            "sources",
            Text::LyricsSources,
            "view-list-symbolic",
            &sources_page::build(&initial, sender.input_sender(), &i18n),
            &i18n,
        );

        let stack = &stack;
        let status = &status;
        let model = Self {
            visible: false,
            draft,
            config_saver,
            status: status.clone(),
            save_tracker,
            language,
        };
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
            SettingsMsg::Change(change) => {
                change.apply(&mut self.draft);
                self.queue_save(&sender);
            }
            SettingsMsg::SaveFinished {
                revision,
                config,
                result,
            } => self.finish_save(revision, config, result, &sender),
            SettingsMsg::LanguageChanged(language) => {
                self.language = language;
                self.render_save_status();
            }
        }
    }
}

impl SettingsModel {
    fn queue_save(&mut self, sender: &ComponentSender<Self>) {
        let config = self.draft.clone();
        let completed_config = Box::new(config.clone());
        let revision = self.save_tracker.begin_save();
        self.render_save_status();
        let input = sender.input_sender().clone();
        self.config_saver.save(config, move |result| {
            let _ = input.send(SettingsMsg::SaveFinished {
                revision,
                config: completed_config,
                result,
            });
        });
    }

    fn finish_save(
        &mut self,
        revision: u64,
        config: Box<AppConfig>,
        result: ConfigSaveResult,
        sender: &ComponentSender<Self>,
    ) {
        match result {
            ConfigSaveResult::Saved => {
                if self.save_tracker.complete(revision, Ok(())) {
                    self.render_save_status();
                }
                // Even a stale successful write is the current on-disk state
                // until a newer queued write completes, so keep the app model
                // synchronized with it.
                let _ = sender.output(SettingsOutput::Saved(config));
            }
            ConfigSaveResult::Failed(error) => {
                if self.save_tracker.complete(revision, Err(error)) {
                    self.render_save_status();
                }
            }
            ConfigSaveResult::Superseded => {}
        }
    }

    fn render_save_status(&self) {
        self.status
            .set_label(&self.save_tracker.render(self.language));
        if self.save_tracker.is_error() {
            self.status.add_css_class("error");
        } else {
            self.status.remove_css_class("error");
        }
    }
}

#[cfg(test)]
use display_page::unplayed_color_sensitive;
#[cfg(test)]
use general_page::{chinese_romanization_index, language_index};

#[cfg(test)]
#[path = "../test/settings_test.rs"]
mod tests;
