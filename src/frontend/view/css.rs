// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Stylesheet constants used by the frontend overlay view.

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
