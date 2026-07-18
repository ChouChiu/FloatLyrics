// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Stylesheet generation and provider lifetime management for the overlay.

use std::{cell::RefCell, rc::Rc};

use crate::shared::config::ConfigLimits;

#[derive(Clone)]
pub(super) struct OverlayStyle {
    opacity_provider: gtk::CssProvider,
    font_provider: gtk::CssProvider,
    font_family: Rc<RefCell<String>>,
}

impl OverlayStyle {
    pub(super) fn install(opacity: f64, font_family: String) -> Self {
        let panel_provider = gtk::CssProvider::new();
        let panel_alpha = opacity.clamp(0.18, 0.72);
        panel_provider.load_from_string(&panel_css(panel_alpha));
        install_provider(&panel_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        let opacity_provider = gtk::CssProvider::new();
        opacity_provider.load_from_string(&opacity_css(opacity));
        install_provider(
            &opacity_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );

        let font_provider = gtk::CssProvider::new();
        font_provider.load_from_string(&font_css(&font_family));
        install_provider(&font_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);

        Self {
            opacity_provider,
            font_provider,
            font_family: Rc::new(RefCell::new(font_family)),
        }
    }

    pub(super) fn apply(&self, opacity: f64, font_family: String) {
        self.opacity_provider
            .load_from_string(&opacity_css(opacity));
        self.font_provider.load_from_string(&font_css(&font_family));
        *self.font_family.borrow_mut() = font_family;
    }

    pub(super) fn font_family(&self) -> String {
        self.font_family.borrow().clone()
    }
}

fn install_provider(provider: &gtk::CssProvider, priority: u32) {
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, provider, priority);
    }
}

fn opacity_css(opacity: f64) -> String {
    let opacity = opacity.clamp(ConfigLimits::OPACITY_MIN, ConfigLimits::OPACITY_MAX);
    format!(".floating-panel {{ background: rgba(10, 12, 16, {opacity:.3}); }}")
}

fn font_css(family: &str) -> String {
    let css_families = family
        .split(',')
        .map(str::trim)
        .map(|name| format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(".floating-panel {{ font-family: {css_families}; }}")
}

/// Returns the main panel CSS string with the given panel alpha value
/// substituted for `__PANEL_ALPHA__`.
pub(super) fn panel_css(panel_alpha: f64) -> String {
    let css = r#"
        window.floating-window,
        window.floating-window > contents,
        .floating-window {
            background: transparent;
            box-shadow: none;
        }

        .floating-panel {
            padding: 7px 14px 8px 14px;
            border: 1px solid rgba(255,255,255,0.10);
            border-radius: 11px;
            background: rgba(10, 12, 16, __PANEL_ALPHA__);
            box-shadow: 0 8px 28px rgba(0,0,0,0.34);
        }

        .floating-song-info {
            color: rgba(255,255,255,0.60);
            font-size: 16px;
            font-weight: 650;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
        }

        .floating-action-button {
            min-width: 20px;
            min-height: 20px;
            padding: 2px;
            color: rgba(255,255,255,0.72);
            transition: 140ms ease;
        }

        .floating-action-button:hover {
            color: white;
            background: rgba(255,255,255,0.12);
        }

        .floating-action-button:active {
            background: rgba(255,255,255,0.20);
        }

        .floating-separator {
            margin: 3px 0 1px 0;
            background: rgba(255,255,255,0.24);
        }

        .floating-panel.snapped-left {
            border-top-left-radius: 0;
            border-bottom-left-radius: 0;
        }

        .floating-panel.snapped-right {
            border-top-right-radius: 0;
            border-bottom-right-radius: 0;
        }

        .floating-panel.snapped-top {
            border-top-left-radius: 0;
            border-top-right-radius: 0;
        }

        .floating-panel.snapped-bottom {
            border-bottom-left-radius: 0;
            border-bottom-right-radius: 0;
        }
        "#;
    css.replace("__PANEL_ALPHA__", &format!("{panel_alpha:.3}"))
}

#[cfg(test)]
#[path = "../../test/view_css_test.rs"]
mod tests;
