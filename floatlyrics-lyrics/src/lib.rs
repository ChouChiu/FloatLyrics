// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

#![warn(missing_docs)]

//! Lyrics-domain services for FloatLyrics.
//!
//! The crate owns provider-neutral lyrics models, parsing, search orchestration,
//! timeline calculations, and persistent caching. It has no GTK or D-Bus
//! dependency.

/// Persistent lyrics cache boundary and SQLite implementation.
pub mod cache;
/// Lyrics models, parsers, provider search, and timeline operations.
pub mod lyrics;
