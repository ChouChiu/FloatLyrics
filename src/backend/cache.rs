// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Serialized SQLite access outside the GTK main thread.

use std::{
    path::Path,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use floatlyrics_core::track::TrackMetadata;
use floatlyrics_lyrics::{
    cache::{Cache, CachedLyrics, LyricsCache, LyricsInsert, ProviderResultInsert},
    lyrics::{FetchedLyrics, LyricsProvider},
};

type LoadCompletion = Box<dyn FnOnce(Result<Option<CachedLyrics>, String>) + Send + 'static>;
type StoreCompletion =
    Box<dyn FnOnce(Result<Option<CachedLyrics>, ProviderStoreError>) + Send + 'static>;
type ApplyCompletion = Box<dyn FnOnce(Result<(), String>) + Send + 'static>;

#[derive(Debug)]
pub(super) enum ProviderStoreError {
    Store(String),
    Load(String),
}

enum CacheCommand {
    RecordTrack {
        track: TrackMetadata,
    },
    LoadTrack {
        track: TrackMetadata,
        provider_order: Vec<LyricsProvider>,
        complete: LoadCompletion,
    },
    StoreProviderAndLoad {
        track_fingerprint: String,
        lyrics: FetchedLyrics,
        provider_order: Vec<LyricsProvider>,
        complete: StoreCompletion,
    },
    ApplyManual {
        track: TrackMetadata,
        lyrics: FetchedLyrics,
        complete: ApplyCompletion,
    },
}

impl CacheCommand {
    fn fail(self, message: String) {
        match self {
            Self::RecordTrack { .. } => {
                tracing::warn!(%message, "failed to queue track cache update")
            }
            Self::LoadTrack { complete, .. } => complete(Err(message)),
            Self::StoreProviderAndLoad { complete, .. } => {
                complete(Err(ProviderStoreError::Store(message)));
            }
            Self::ApplyManual { complete, .. } => complete(Err(message)),
        }
    }
}

/// Cloneable command port for the cache worker.
#[derive(Clone)]
pub(super) struct CacheService {
    sender: mpsc::Sender<CacheCommand>,
}

impl CacheService {
    pub(super) fn record_track(&self, track: TrackMetadata) {
        self.send(CacheCommand::RecordTrack { track });
    }

    pub(super) fn load_track(
        &self,
        track: TrackMetadata,
        provider_order: Vec<LyricsProvider>,
        complete: impl FnOnce(Result<Option<CachedLyrics>, String>) + Send + 'static,
    ) {
        self.send(CacheCommand::LoadTrack {
            track,
            provider_order,
            complete: Box::new(complete),
        });
    }

    pub(super) fn store_provider_and_load(
        &self,
        track_fingerprint: String,
        lyrics: FetchedLyrics,
        provider_order: Vec<LyricsProvider>,
        complete: impl FnOnce(Result<Option<CachedLyrics>, ProviderStoreError>) + Send + 'static,
    ) {
        self.send(CacheCommand::StoreProviderAndLoad {
            track_fingerprint,
            lyrics,
            provider_order,
            complete: Box::new(complete),
        });
    }

    pub(super) fn apply_manual(
        &self,
        track: TrackMetadata,
        lyrics: FetchedLyrics,
        complete: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        self.send(CacheCommand::ApplyManual {
            track,
            lyrics,
            complete: Box::new(complete),
        });
    }

    fn send(&self, command: CacheCommand) {
        if let Err(error) = self.sender.send(command) {
            error
                .0
                .fail("lyrics cache worker stopped unexpectedly".to_string());
        }
    }
}

/// Owns the SQLite connection thread for one backend instance.
pub(super) struct CacheWorker {
    service: Option<CacheService>,
    worker: Option<JoinHandle<()>>,
}

impl CacheWorker {
    pub(super) fn new(database_file: &Path) -> Result<Self> {
        let cache = Cache::open(database_file)?;
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("floatlyrics-cache".to_string())
            .spawn(move || run_worker(&cache, receiver))
            .context("spawning lyrics cache worker")?;
        Ok(Self {
            service: Some(CacheService { sender }),
            worker: Some(worker),
        })
    }

    pub(super) fn service(&self) -> CacheService {
        self.service
            .as_ref()
            .expect("cache worker service is available before drop")
            .clone()
    }
}

impl Drop for CacheWorker {
    fn drop(&mut self) {
        self.service.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::warn!("lyrics cache worker panicked");
        }
    }
}

fn run_worker(cache: &dyn LyricsCache, receiver: mpsc::Receiver<CacheCommand>) {
    while let Ok(command) = receiver.recv() {
        match command {
            CacheCommand::RecordTrack { track } => {
                if let Err(error) = cache.upsert_track(&track) {
                    tracing::warn!(%error, "failed to cache playback track");
                }
            }
            CacheCommand::LoadTrack {
                track,
                provider_order,
                complete,
            } => complete(load_track(cache, &track, &provider_order)),
            CacheCommand::StoreProviderAndLoad {
                track_fingerprint,
                lyrics,
                provider_order,
                complete,
            } => complete(store_provider_and_load(
                cache,
                &track_fingerprint,
                &lyrics,
                &provider_order,
            )),
            CacheCommand::ApplyManual {
                track,
                lyrics,
                complete,
            } => complete(apply_manual(cache, &track, &lyrics)),
        }
    }
}

fn load_track(
    cache: &dyn LyricsCache,
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Option<CachedLyrics>, String> {
    let fingerprint = cache
        .upsert_track(track)
        .map_err(|error| format!("{error:#}"))?;
    cache
        .lyrics_for_track(&fingerprint, provider_order)
        .map_err(|error| format!("{error:#}"))
}

fn store_provider_and_load(
    cache: &dyn LyricsCache,
    track_fingerprint: &str,
    lyrics: &FetchedLyrics,
    provider_order: &[LyricsProvider],
) -> Result<Option<CachedLyrics>, ProviderStoreError> {
    cache
        .insert_provider_result(ProviderResultInsert {
            track_fingerprint,
            provider: lyrics.provider,
            provider_track_id: lyrics.provider_track_id.as_deref(),
            title: &lyrics.title,
            artists: &lyrics.artists,
            score: lyrics.score,
            raw_lyrics: Some(&lyrics.raw_lyrics),
        })
        .map_err(|error| ProviderStoreError::Store(format!("{error:#}")))?;
    cache
        .lyrics_for_track(track_fingerprint, provider_order)
        .map_err(|error| ProviderStoreError::Load(format!("{error:#}")))
}

fn apply_manual(
    cache: &dyn LyricsCache,
    track: &TrackMetadata,
    lyrics: &FetchedLyrics,
) -> Result<(), String> {
    let track_fingerprint = cache
        .upsert_track(track)
        .map_err(|error| format!("{error:#}"))?;
    let lyrics_id = cache
        .insert_lyrics(LyricsInsert {
            provider: lyrics.provider,
            provider_track_id: lyrics.provider_track_id.as_deref(),
            title: &lyrics.title,
            artists: &lyrics.artists,
            raw_lyrics: &lyrics.raw_lyrics,
        })
        .map_err(|error| format!("{error:#}"))?;
    cache
        .bind_manual_match(&track_fingerprint, lyrics_id)
        .map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
#[path = "../test/cache_service_test.rs"]
mod tests;
