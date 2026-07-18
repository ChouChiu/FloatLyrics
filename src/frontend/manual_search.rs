// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Track-specific manual lyrics search and selection frontend.

mod session;
mod state;
mod view;

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use crate::{
    backend::{ControllerHandle, ManualSearchService},
    shared::manual_search::{FetchedLyrics, LyricsCandidate},
};
use floatlyrics_core::{
    i18n::{I18n, Language, Text},
    track::TrackMetadata,
};

use super::localization::{
    bind_button_label, bind_entry_placeholder, bind_label, bind_window_title,
};
use session::{ManualSearchSession, SearchWidgets};
use view::install_css;

const WINDOW_WIDTH: i32 = 820;
const WINDOW_HEIGHT: i32 = 560;

#[derive(Debug)]
pub(super) enum SearchEvent {
    Candidates {
        generation: u64,
        result: Result<Vec<LyricsCandidate>, String>,
    },
    Preview {
        generation: u64,
        index: usize,
        result: Result<Option<FetchedLyrics>, String>,
    },
    Applied {
        generation: u64,
        target_fingerprint: String,
        result: Result<(), String>,
    },
}

pub(super) struct ManualSearchInit {
    pub(super) service: ManualSearchService,
    pub(super) controller: ControllerHandle,
    pub(super) i18n: I18n,
}

pub(super) struct ManualSearchModel {
    visible: bool,
    session: ManualSearchSession,
}

#[derive(Debug)]
pub(super) enum ManualSearchMsg {
    Show,
    Hide,
    Search,
    Apply,
    SelectRow(Option<usize>),
    LanguageChanged(Language),
    Event(SearchEvent),
}

#[relm4::component(pub(super))]
impl SimpleComponent for ManualSearchModel {
    type Init = ManualSearchInit;
    type Input = ManualSearchMsg;
    type Output = ();

    view! {
        window = gtk::ApplicationWindow {
            set_application: Some(&relm4::main_application()),
            set_default_size: (WINDOW_WIDTH, WINDOW_HEIGHT),
            set_resizable: false,
            set_hide_on_close: true,
            #[watch]
            set_visible: model.visible,

            #[wrap(Some)]
            set_titlebar = &gtk::WindowHandle {
                #[wrap(Some)]
                set_child = &gtk::HeaderBar {
                    set_show_title_buttons: true,
                    #[wrap(Some)]
                    #[name = "header_title"]
                    set_title_widget = &gtk::Label {
                        set_label: "",
                    },
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    add_css_class: "manual-search-bar",
                    #[name = "title_label"]
                    gtk::Label {},
                    #[local_ref]
                    title_widget -> gtk::Entry {
                        connect_activate[sender] => move |_| sender.input(ManualSearchMsg::Search),
                    },
                    #[name = "artist_label"]
                    gtk::Label {},
                    #[local_ref]
                    artist_widget -> gtk::Entry {
                        connect_activate[sender] => move |_| sender.input(ManualSearchMsg::Search),
                    },
                    #[local_ref]
                    spinner_widget -> gtk::Spinner {},
                    #[local_ref]
                    search_button_widget -> gtk::Button {
                        connect_clicked[sender] => move |_| sender.input(ManualSearchMsg::Search),
                    },
                },

                gtk::Separator { set_orientation: gtk::Orientation::Horizontal },
                gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 360,
                    set_wide_handle: true,
                    set_vexpand: true,
                    #[wrap(Some)]
                    set_start_child = &gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        set_min_content_width: 340,
                        #[local_ref]
                        results_widget -> gtk::ListBox {},
                    },
                    #[wrap(Some)]
                    set_end_child = &gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        set_hexpand: true,
                        #[local_ref]
                        preview_widget -> gtk::TextView {},
                    },
                },
                gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    add_css_class: "manual-search-footer",
                    #[local_ref]
                    status_widget -> gtk::Label {},
                    #[local_ref]
                    apply_widget -> gtk::Button {
                        connect_clicked[sender] => move |_| sender.input(ManualSearchMsg::Apply),
                    },
                },
            },

            connect_close_request[sender] => move |_| {
                sender.input(ManualSearchMsg::Hide);
                gtk::glib::Propagation::Proceed
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let ManualSearchInit {
            service,
            controller,
            i18n,
        } = init;
        let title = gtk::Entry::builder().hexpand(true).build();
        bind_entry_placeholder(&title, &i18n, Text::SongTitle);
        let artist = gtk::Entry::builder().hexpand(true).build();
        bind_entry_placeholder(&artist, &i18n, Text::Artist);
        let search_button = gtk::Button::builder()
            .css_classes(["suggested-action"])
            .build();
        bind_button_label(&search_button, &i18n, Text::Search);
        let spinner = gtk::Spinner::new();

        let results = gtk::ListBox::new();
        results.set_selection_mode(gtk::SelectionMode::Single);
        results.add_css_class("boxed-list");

        let preview = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(12)
            .right_margin(12)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        preview.add_css_class("manual-preview");
        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let apply = gtk::Button::builder()
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        bind_button_label(&apply, &i18n, Text::ApplySelectedLyrics);
        install_css();
        let session = ManualSearchSession::new(
            service,
            controller,
            &i18n,
            sender.input_sender().clone(),
            SearchWidgets {
                title: title.clone(),
                artist: artist.clone(),
                results: results.clone(),
                preview: preview.clone(),
                status: status.clone(),
                apply: apply.clone(),
                spinner: spinner.clone(),
                search_button: search_button.clone(),
            },
        );
        let model = Self {
            visible: false,
            session,
        };
        let title_widget = &title;
        let artist_widget = &artist;
        let spinner_widget = &spinner;
        let search_button_widget = &search_button;
        let results_widget = &results;
        let preview_widget = &preview;
        let status_widget = &status;
        let apply_widget = &apply;
        let widgets = view_output!();
        bind_label(&widgets.header_title, &i18n, Text::ManualSearchTitle);
        bind_label(&widgets.title_label, &i18n, Text::Title);
        bind_label(&widgets.artist_label, &i18n, Text::Artist);
        bind_window_title(&root, &i18n, Text::ManualSearchTitle);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            ManualSearchMsg::Show => {
                self.session.prepare_for_show();
                self.visible = true;
                self.session.start_search();
            }
            ManualSearchMsg::Hide => self.visible = false,
            ManualSearchMsg::Search => self.session.start_search(),
            ManualSearchMsg::Apply => self.session.apply_selected(),
            ManualSearchMsg::SelectRow(index) => self.session.select_row(index),
            ManualSearchMsg::LanguageChanged(language) => self.session.relocalize(language),
            ManualSearchMsg::Event(event) => self.session.handle_event(event),
        }
    }
}

fn search_field_values(track: &TrackMetadata) -> (String, String) {
    ManualSearchService::search_field_values(track)
}

#[cfg(test)]
#[path = "../test/manual_search_test.rs"]
mod tests;
