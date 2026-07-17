// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Stable cryptographic digests shared by domain and persistence code.

use sha2::{Digest, Sha256};

/// Returns the lowercase hexadecimal SHA-256 digest of `content`.
///
/// # Examples
///
/// ```
/// use floatlyrics_core::digest::sha256_hex;
///
/// assert_eq!(
///     sha256_hex(b"FloatLyrics"),
///     "4253a2e893a46c5ddc15dc27c8d056bf863858638f6778af3f49afe67494c9b1"
/// );
/// ```
pub fn sha256_hex(content: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_ref());
    format!("{:x}", hasher.finalize())
}
