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

            #[name = "drag_handle"]
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

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 0,
                        set_valign: gtk::Align::Center,
                        add_css_class: "floating-offset-control",

                        #[name = "offset_decrease_button"]
                        gtk::Button {
                            set_valign: gtk::Align::Center,
                            set_css_classes: &["flat", "floating-offset-step-button"],
                            connect_clicked[sender = init.sender.clone()] => move |_| {
                                let _ = sender.send(AppMsg::AdjustTrackOffset(-100));
                            },
                        },
                        #[name = "track_offset_button"]
                        gtk::Button {
                            set_label: "",
                            set_valign: gtk::Align::Center,
                            set_css_classes: &["flat", "floating-offset-button"],
                            connect_clicked[sender = init.sender.clone()] => move |_| {
                                let _ = sender.send(AppMsg::ResetTrackOffset);
                            },
                        },
                        #[name = "offset_increase_button"]
                        gtk::Button {
                            set_valign: gtk::Align::Center,
                            set_css_classes: &["flat", "floating-offset-step-button"],
                            connect_clicked[sender = init.sender.clone()] => move |_| {
                                let _ = sender.send(AppMsg::AdjustTrackOffset(100));
                            },
                        },
                    },
                    #[name = "manual_search_button"]
                    gtk::Button {
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenManualSearch);
                        },
                    },
                    #[name = "settings_button"]
                    gtk::Button {
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenSettings);
                        },
                    },
                    #[name = "close_button"]
                    gtk::Button {
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
    pub(super) drag_handle: gtk::Box,
    pub(super) song_info: gtk::Label,
    pub(super) offset_decrease_button: gtk::Button,
    pub(super) track_offset_button: gtk::Button,
    pub(super) offset_increase_button: gtk::Button,
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
    set_button_icon(&panel.offset_decrease_button, PanelIcon::Minus);
    set_button_icon(&panel.offset_increase_button, PanelIcon::Plus);
    set_button_icon(&panel.manual_search_button, PanelIcon::Search);
    set_button_icon(&panel.settings_button, PanelIcon::Settings);
    set_button_icon(&panel.close_button, PanelIcon::Close);
    PanelWidgets {
        content: panel.content.clone(),
        drag_handle: panel.drag_handle.clone(),
        song_info: panel.song_info.clone(),
        offset_decrease_button: panel.offset_decrease_button.clone(),
        track_offset_button: panel.track_offset_button.clone(),
        offset_increase_button: panel.offset_increase_button.clone(),
        manual_search_button: panel.manual_search_button.clone(),
        settings_button: panel.settings_button.clone(),
        close_button: panel.close_button.clone(),
        lyrics_viewport: panel.lyrics_viewport.clone(),
    }
}

#[derive(Clone, Copy)]
enum PanelIcon {
    Minus,
    Plus,
    Search,
    Settings,
    Close,
}

fn set_button_icon(button: &gtk::Button, icon: PanelIcon) {
    let drawing = gtk::DrawingArea::new();
    drawing.set_content_width(16);
    drawing.set_content_height(16);
    drawing.set_draw_func(move |drawing, cr, width, height| {
        let color = drawing.color();
        cr.set_source_rgba(
            color.red().into(),
            color.green().into(),
            color.blue().into(),
            color.alpha().into(),
        );
        cr.scale(width as f64 / 16.0, height as f64 / 16.0);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.set_line_width(1.6);

        match icon {
            PanelIcon::Minus => {
                cr.move_to(3.0, 8.0);
                cr.line_to(13.0, 8.0);
            }
            PanelIcon::Plus => {
                cr.move_to(3.0, 8.0);
                cr.line_to(13.0, 8.0);
                cr.move_to(8.0, 3.0);
                cr.line_to(8.0, 13.0);
            }
            PanelIcon::Search => {
                cr.arc(6.75, 6.75, 4.25, 0.0, std::f64::consts::TAU);
                cr.move_to(9.8, 9.8);
                cr.line_to(13.5, 13.5);
            }
            PanelIcon::Settings => {
                for (y, knob_x) in [(4.0, 6.0), (8.0, 10.0), (12.0, 5.0)] {
                    cr.move_to(2.5, y);
                    cr.line_to(13.5, y);
                    cr.move_to(knob_x, y - 1.7);
                    cr.line_to(knob_x, y + 1.7);
                }
            }
            PanelIcon::Close => {
                cr.move_to(4.0, 4.0);
                cr.line_to(12.0, 12.0);
                cr.move_to(12.0, 4.0);
                cr.line_to(4.0, 12.0);
            }
        }
        let _ = cr.stroke();
    });
    button.set_child(Some(&drawing));
}
