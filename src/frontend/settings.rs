// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Configuration persistence used by the React settings frontend.

pub(super) mod font_picker;
mod persistence;
pub(super) mod window;

pub(in crate::frontend) use persistence::{ConfigSaveResult, ConfigSaveService};
