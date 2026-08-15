// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Configuration persistence used by the React settings frontend.

mod persistence;

pub(in crate::frontend) use persistence::{ConfigSaveResult, ConfigSaveService};
