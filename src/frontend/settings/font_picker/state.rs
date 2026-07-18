// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure ordered font selection rules.

pub(super) struct FontSelection {
    fonts: Vec<String>,
}

impl FontSelection {
    pub(super) fn new(fonts: Vec<String>) -> Self {
        Self { fonts }
    }

    #[cfg(test)]
    pub(super) fn fonts(&self) -> &[String] {
        &self.fonts
    }

    pub(super) fn into_fonts(self) -> Vec<String> {
        self.fonts
    }

    pub(super) fn add(&mut self, family: String) -> bool {
        if family.trim().is_empty() || self.fonts.contains(&family) {
            return false;
        }
        self.fonts.push(family);
        true
    }

    pub(super) fn move_by(&mut self, index: usize, delta: isize) -> bool {
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if index >= self.fonts.len() || target >= self.fonts.len() || index == target {
            return false;
        }
        self.fonts.swap(index, target);
        true
    }

    pub(super) fn remove(&mut self, index: usize) -> bool {
        if self.fonts.len() <= 1 || index >= self.fonts.len() {
            return false;
        }
        self.fonts.remove(index);
        true
    }
}

#[cfg(test)]
#[path = "../../../test/font_selection_test.rs"]
mod tests;
