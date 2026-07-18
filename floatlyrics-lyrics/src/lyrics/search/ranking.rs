// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-order and candidate-ranking policy.

use std::{cmp::Reverse, collections::HashSet};

use crate::lyrics::model::{LyricsCandidate, LyricsProvider};

const MANUAL_SEARCH_LIMIT: usize = 12;

pub(super) fn finalize_candidates(mut candidates: Vec<LyricsCandidate>) -> Vec<LyricsCandidate> {
    candidates.sort_by_key(|candidate| Reverse(candidate.match_score));
    let mut seen = HashSet::new();
    candidates.retain(|candidate| {
        seen.insert((
            candidate.provider.as_str(),
            candidate.provider_track_id.clone(),
        ))
    });
    candidates.truncate(MANUAL_SEARCH_LIMIT);
    candidates
}

/// Validated provider priority used by automatic search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    providers: Vec<LyricsProvider>,
}

impl SearchPlan {
    /// Builds a plan, removing repeated providers.
    pub fn new(providers: impl IntoIterator<Item = LyricsProvider>) -> Self {
        let mut providers = providers.into_iter().collect::<Vec<_>>();
        let mut seen = HashSet::new();
        providers.retain(|provider| seen.insert(*provider));
        Self { providers }
    }

    /// Builds the default QQ Music then NetEase plan.
    pub fn default_mvp() -> Self {
        Self::new(LyricsProvider::default_order())
    }

    /// Returns providers in search priority order.
    pub fn providers(&self) -> &[LyricsProvider] {
        &self.providers
    }
}
