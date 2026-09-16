// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Resolution of MPRIS cover URLs into AMLL protocol cover sources.

use std::path::PathBuf;

use super::Cover;

/// Classifies a player-provided cover URL.
///
/// Local files are streamed over the protocol's binary channel because
/// listeners cannot fetch `file://` URLs; every other URL is forwarded as is.
pub(super) fn cover_source(url: &str) -> Cover {
    let trimmed = url.trim();
    match local_cover_path(trimmed) {
        Some(path) => Cover::File(path),
        None => Cover::Uri(trimmed.to_string()),
    }
}

/// Resolves `file://` URLs to a local path, decoding percent escapes.
///
/// Returns `None` for remote hosts, for relative paths, and for escapes that
/// do not form valid UTF-8.
fn local_cover_path(url: &str) -> Option<PathBuf> {
    let remainder = url.strip_prefix("file://")?;
    let path = match remainder.find('/') {
        Some(0) => remainder,
        Some(index) if &remainder[..index] == "localhost" => &remainder[index..],
        _ => return None,
    };
    percent_decode(path).map(PathBuf::from)
}

fn percent_decode(value: &str) -> Option<String> {
    if !value.contains('%') {
        return Some(value.to_string());
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
#[path = "../../test/amll_cover_test.rs"]
mod tests;
