// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

#![warn(missing_docs)]

//! Platform-independent foundations for FloatLyrics.
//!
//! This crate contains stable domain metadata, user-data path resolution,
//! runtime localization, and telemetry setup. It intentionally has no GTK or
//! D-Bus dependency.

/// Stable cryptographic digest helpers.
pub mod digest;
/// Runtime localization and the compiled translation catalogue.
pub mod i18n;
/// User configuration and data path resolution.
pub mod paths;
/// Process-wide tracing initialization.
pub mod telemetry;
/// Playback track metadata and stable fingerprinting.
pub mod track;
