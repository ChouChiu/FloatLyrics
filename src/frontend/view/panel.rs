// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Static GTK widget template for the floating overlay panel.

use gtk::prelude::*;
use relm4::WidgetTemplate;

use super::super::AppMsg;

struct OverlayPanelInit {
    width: i32,
    viewport_height: i32,
    sender: relm4::Sender<AppMsg>,
}

#[relm4::widget_template]
impl WidgetTemplate for OverlayPanel {
    type Init = OverlayPanelInit;

    view! {
        #[name = "content"]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Center,
            set_size_request: (init.width, -1),
            add_css_class: "floating-panel",

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,

                #[name = "song_info"]
                gtk::Label {
                    set_label: "FloatLyrics",
                    set_halign: gtk::Align::Start,
                    set_valign: gtk::Align::Center,
                    set_hexpand: true,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 48,
                    set_single_line_mode: true,
                    add_css_class: "floating-song-info",
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    #[name = "manual_search_button"]
                    gtk::Button {
                        set_icon_name: "system-search-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenManualSearch);
                        },
                    },
                    #[name = "settings_button"]
                    gtk::Button {
                        set_icon_name: "emblem-system-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenSettings);
                        },
                    },
                    #[name = "close_button"]
                    gtk::Button {
                        set_icon_name: "window-close-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::Quit);
                        },
                    },
                },
            },

            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
                add_css_class: "floating-separator",
            },

            #[name = "lyrics_viewport"]
            gtk::Box {
                set_size_request: (init.width, init.viewport_height),
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
            },
        }
    }
}

pub(super) struct PanelWidgets {
    pub(super) content: gtk::Box,
    pub(super) song_info: gtk::Label,
    pub(super) manual_search_button: gtk::Button,
    pub(super) settings_button: gtk::Button,
    pub(super) close_button: gtk::Button,
    pub(super) lyrics_viewport: gtk::Box,
}

pub(super) fn build(
    width: i32,
    viewport_height: i32,
    sender: relm4::Sender<AppMsg>,
) -> PanelWidgets {
    let panel = OverlayPanel::init(OverlayPanelInit {
        width,
        viewport_height,
        sender,
    });
    PanelWidgets {
        content: panel.content.clone(),
        song_info: panel.song_info.clone(),
        manual_search_button: panel.manual_search_button.clone(),
        settings_button: panel.settings_button.clone(),
        close_button: panel.close_button.clone(),
        lyrics_viewport: panel.lyrics_viewport.clone(),
    }
}
