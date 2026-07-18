// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! GTK presentation helpers for manual-search results.

use floatlyrics_core::i18n::{Language, Text};
use gtk::prelude::*;

use crate::shared::manual_search::{LyricsCandidate, LyricsProvider};

pub(super) fn candidate_row(candidate: &LyricsCandidate, language: Language) -> gtk::ListBoxRow {
    let title = gtk::Label::builder()
        .label(&candidate.title)
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .build();
    let detail = gtk::Label::builder()
        .label(format!(
            "{}  ·  {}  ·  {}",
            candidate.artists.join(", "),
            provider_name(candidate.provider, language),
            duration_text(candidate.duration_ms)
        ))
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label"])
        .build();
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
    labels.set_hexpand(true);
    labels.append(&title);
    labels.append(&detail);
    let raw_score = candidate.match_score;
    if raw_score < 0 {
        tracing::warn!(raw_score, "negative match score clamped to 0");
    }
    let score = gtk::Label::builder()
        .label(format!("{}%", raw_score.max(0)))
        .valign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    score.set_tooltip_text(Some(language.text(Text::MatchScore)));
    let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row_content.add_css_class("manual-result-row");
    row_content.append(&labels);
    row_content.append(&score);
    gtk::ListBoxRow::builder().child(&row_content).build()
}

fn provider_name(provider: LyricsProvider, language: Language) -> &'static str {
    match provider {
        LyricsProvider::QqMusic => language.text(Text::ProviderNameQqMusic),
        LyricsProvider::NetEase => language.text(Text::ProviderNameNetEase),
    }
}

fn duration_text(duration_ms: Option<i32>) -> String {
    let seconds = duration_ms.unwrap_or_default().max(0) / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub(super) fn install_css() {
    crate::frontend::style::install(
        r#"
        .manual-search-bar, .manual-search-footer { padding: 12px; }
        .manual-result-row { padding: 10px 12px; }
        "#,
    );
}

#[cfg(test)]
#[path = "../../test/manual_search_view_test.rs"]
mod tests;
