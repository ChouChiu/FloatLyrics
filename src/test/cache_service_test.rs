use super::*;
use std::time::Duration;

#[test]
fn worker_serializes_load_store_and_manual_selection() {
    let directory = tempfile::tempdir().unwrap();
    let worker = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let service = worker.service();
    let track = track();
    let fingerprint = track.fingerprint();
    let providers = vec![LyricsProvider::QqMusic, LyricsProvider::NetEase];

    let (sender, receiver) = mpsc::channel();
    service.load_track(track.clone(), providers.clone(), move |result| {
        sender.send(result).unwrap();
    });
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(CachedTrack {
            lyrics: None,
            offset_ms: 0,
        })
    );

    let (sender, receiver) = mpsc::channel();
    service.store_provider_and_load(
        fingerprint,
        fetched("provider"),
        providers.clone(),
        move |result| sender.send(result).unwrap(),
    );
    let cached = receiver
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(cached.raw_lyrics, "provider");
    assert!(!cached.manually_selected);

    let (sender, receiver) = mpsc::channel();
    service.apply_manual(track.clone(), fetched("manual"), move |result| {
        sender.send(result).unwrap();
    });
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );

    let (sender, receiver) = mpsc::channel();
    service.load_track(track, providers, move |result| {
        let worker_name = thread::current().name().map(str::to_string);
        sender.send((worker_name, result)).unwrap();
    });
    let (worker_name, cached) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(worker_name.as_deref(), Some("floatlyrics-cache"));
    let cached = cached.unwrap().lyrics.unwrap();
    assert_eq!(cached.raw_lyrics, "manual");
    assert!(cached.manually_selected);
}

#[test]
fn dropping_worker_flushes_queued_commands() {
    let directory = tempfile::tempdir().unwrap();
    let worker = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let service = worker.service();
    let (sender, receiver) = mpsc::channel();
    let track = track();

    service.record_track(track.clone());
    service.apply_manual(track, fetched("manual"), move |result| {
        sender.send(result).unwrap();
    });
    drop(service);
    drop(worker);

    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );
}

#[test]
fn worker_persists_per_track_offset_with_the_track() {
    let directory = tempfile::tempdir().unwrap();
    let worker = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let service = worker.service();
    let track = track();
    let (sender, receiver) = mpsc::channel();

    service.set_track_offset(track.clone(), 450, move |result| {
        sender.send(result).unwrap();
    });
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );

    let (sender, receiver) = mpsc::channel();
    service.load_track(track, Vec::new(), move |result| {
        sender.send(result).unwrap();
    });
    let loaded = receiver
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.offset_ms, 450);
}

fn track() -> TrackMetadata {
    TrackMetadata {
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        album: None,
        duration_ms: Some(60_000),
        mpris_track_id: None,
    }
}

fn fetched(raw_lyrics: &str) -> FetchedLyrics {
    FetchedLyrics {
        provider: LyricsProvider::QqMusic,
        provider_track_id: Some(raw_lyrics.to_string()),
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        score: 100.0,
        raw_lyrics: raw_lyrics.to_string(),
    }
}
