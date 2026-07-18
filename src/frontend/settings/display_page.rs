// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Window geometry, typography, and color settings page.

use std::rc::Rc;

use floatlyrics_core::i18n::{I18n, Text};
use gtk::prelude::*;

use crate::shared::config::{AppConfig, ConfigLimits};

use super::font_picker::font_window;
use super::view::{color_row, connect_window_i32, page, setting_card, setting_row};
use super::{ConfigChange, SettingsMsg};

pub(super) fn build(
    config: &AppConfig,
    sender: &relm4::Sender<SettingsMsg>,
    i18n: &I18n,
) -> gtk::ScrolledWindow {
    let apple_music_style = gtk::Switch::builder()
        .active(config.lyrics.apple_music_style)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        apple_music_style.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::AppleMusicStyle(
                input.is_active(),
            )));
        });
    }

    let fonts = gtk::Button::with_label("");
    fonts.set_width_request(190);
    {
        let fonts = fonts.clone();
        i18n.subscribe(move |language| fonts.set_label(language.text(Text::ChangeFonts)));
    }
    {
        let persist: Rc<dyn Fn(Vec<String>)> = {
            let sender = sender.clone();
            Rc::new(move |fonts| {
                let _ = sender.send(SettingsMsg::Change(ConfigChange::Fonts(fonts)));
            })
        };
        let picker_window = font_window(config.lyrics.font_order.clone(), &persist, i18n);
        fonts.connect_clicked(move |_| picker_window.present());
    }

    let width = gtk::SpinButton::with_range(
        ConfigLimits::WINDOW_WIDTH_MIN.into(),
        ConfigLimits::WINDOW_WIDTH_MAX.into(),
        10.0,
    );
    width.set_value(config.window.width as f64);
    width.set_numeric(true);
    width.set_width_chars(8);
    connect_window_i32(&width, sender, ConfigChange::Width);

    let remember_position = gtk::Switch::builder()
        .active(config.window.remember_position)
        .valign(gtk::Align::Center)
        .build();
    {
        let sender = sender.clone();
        remember_position.connect_active_notify(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::RememberPosition(
                input.is_active(),
            )));
        });
    }

    let margin = gtk::SpinButton::with_range(
        ConfigLimits::WINDOW_MARGIN_MIN.into(),
        ConfigLimits::WINDOW_MARGIN_MAX.into(),
        4.0,
    );
    margin.set_value(config.window.margin as f64);
    margin.set_numeric(true);
    margin.set_width_chars(8);
    connect_window_i32(&margin, sender, ConfigChange::Margin);

    let panel_height = gtk::SpinButton::with_range(
        ConfigLimits::BOTTOM_PANEL_HEIGHT_MIN.into(),
        ConfigLimits::BOTTOM_PANEL_HEIGHT_MAX.into(),
        2.0,
    );
    panel_height.set_value(config.window.bottom_panel_height as f64);
    panel_height.set_numeric(true);
    panel_height.set_width_chars(8);
    connect_window_i32(&panel_height, sender, ConfigChange::PanelHeight);

    let opacity = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        ConfigLimits::OPACITY_MIN,
        ConfigLimits::OPACITY_MAX,
        0.01,
    );
    opacity.set_value(
        config
            .window
            .opacity
            .clamp(ConfigLimits::OPACITY_MIN, ConfigLimits::OPACITY_MAX),
    );
    opacity.set_draw_value(true);
    opacity.set_digits(2);
    opacity.set_width_request(200);
    {
        let sender = sender.clone();
        opacity.connect_value_changed(move |input| {
            let _ = sender.send(SettingsMsg::Change(ConfigChange::Opacity(input.value())));
        });
    }

    let lyric_font_size = gtk::SpinButton::with_range(
        ConfigLimits::LYRIC_FONT_SIZE_MIN.into(),
        ConfigLimits::LYRIC_FONT_SIZE_MAX.into(),
        1.0,
    );
    lyric_font_size.set_value(config.lyrics.lyric_font_size as f64);
    lyric_font_size.set_numeric(true);
    lyric_font_size.set_width_chars(8);
    connect_window_i32(&lyric_font_size, sender, ConfigChange::LyricFontSize);

    let translation_font_size = gtk::SpinButton::with_range(
        ConfigLimits::SECONDARY_FONT_SIZE_MIN.into(),
        ConfigLimits::SECONDARY_FONT_SIZE_MAX.into(),
        1.0,
    );
    translation_font_size.set_value(config.lyrics.translation_font_size as f64);
    translation_font_size.set_numeric(true);
    translation_font_size.set_width_chars(8);
    connect_window_i32(
        &translation_font_size,
        sender,
        ConfigChange::TranslationFontSize,
    );

    let romanization_font_size = gtk::SpinButton::with_range(
        ConfigLimits::SECONDARY_FONT_SIZE_MIN.into(),
        ConfigLimits::SECONDARY_FONT_SIZE_MAX.into(),
        1.0,
    );
    romanization_font_size.set_value(config.lyrics.romanization_font_size as f64);
    romanization_font_size.set_numeric(true);
    romanization_font_size.set_width_chars(8);
    connect_window_i32(
        &romanization_font_size,
        sender,
        ConfigChange::RomanizationFontSize,
    );

    let unplayed_color = color_row(
        i18n,
        Text::UnplayedColor,
        Text::UnplayedColorDescription,
        &config.lyrics.unplayed_color,
        sender.clone(),
        ConfigChange::UnplayedColor,
    );
    unplayed_color.set_sensitive(unplayed_color_sensitive(config.lyrics.apple_music_style));
    {
        let unplayed_color = unplayed_color.clone();
        apple_music_style.connect_active_notify(move |input| {
            unplayed_color.set_sensitive(unplayed_color_sensitive(input.is_active()));
        });
    }

    page(
        i18n,
        Text::DisplayTitle,
        Text::DisplayDescription,
        &[
            setting_card(&[
                setting_row(
                    i18n,
                    Text::AppleMusicStyle,
                    Text::AppleMusicStyleDescription,
                    &apple_music_style,
                ),
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
                    ConfigChange::PlayedColor,
                ),
                unplayed_color,
                color_row(
                    i18n,
                    Text::TranslationColor,
                    Text::TranslationColorDescription,
                    &config.lyrics.translation_color,
                    sender.clone(),
                    ConfigChange::TranslationColor,
                ),
                color_row(
                    i18n,
                    Text::RomanizationColor,
                    Text::RomanizationColorDescription,
                    &config.lyrics.romanization_color,
                    sender.clone(),
                    ConfigChange::RomanizationColor,
                ),
            ]),
        ],
    )
}

pub(super) fn unplayed_color_sensitive(apple_music_style: bool) -> bool {
    !apple_music_style
}
