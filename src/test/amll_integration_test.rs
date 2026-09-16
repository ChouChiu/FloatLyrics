use super::*;

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use floatlyrics_core::track::TrackMetadata;
use floatlyrics_lyrics::lyrics::{FetchedLyrics, LyricsProvider};
use futures_util::StreamExt;
use serde_json::Value;

use crate::backend::{
    Controller,
    cache::CacheWorker,
    mpris::{PlaybackStatus, PlayerState, PlayerWatcherEvent},
};
use crate::shared::{
    config::AppConfig,
    presentation::{LoopStatus, PlayerControl},
    runtime::LyricsRuntimeConfig,
};

/// Word-timed Korean lyrics whose readings exercise the per-word mapping. The
/// `[start,duration]` line tag and the absolute `(start,duration)` word tags are
/// QRC syntax, which is what carries word timing.
const CACHED_LYRICS: &str = "[1000,2000]안녕(1000,400) 세계(1400,400)";
/// Line-timed lyrics: no provider word timing, so every line is segmented and
/// its own duration is distributed over the tokens.
const LINE_TIMED_LYRICS: &str = "[00:01.00]안녕 세계\n[00:03.00]你好世界\n[00:05.00]bye\n";
/// Wait for silence after the last protocol message before asserting.
const IDLE: Duration = Duration::from_millis(600);

/// Runs the real controller with an [`AmllSender`] over a real socket and
/// returns every protocol message a listening AMLL player received.
///
/// `raw_lyrics` is stored for the track before the controller starts — `None`
/// publishes a track that has no lyrics at all — and `track_offset_ms` is applied
/// to the connected track, so a test can tell the lyric clock and the playback
/// clock apart.
fn publish(raw_lyrics: Option<&str>, track_offset_ms: i64) -> Vec<Value> {
    publish_with(raw_lyrics, &["Artist"], &["Artist"], track_offset_ms)
}

/// [`publish`] with the player's own billing and the provider's kept apart, so a
/// test can tell the artists the player reports from the ones the lyrics credit.
fn publish_with(
    raw_lyrics: Option<&str>,
    track_artists: &[&str],
    cached_artists: &[&str],
    track_offset_ms: i64,
) -> Vec<Value> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let listener = runtime
        .block_on(async { tokio::net::TcpListener::bind("127.0.0.1:0").await })
        .expect("listener");
    let address = listener.local_addr().expect("address").to_string();
    let player = runtime.spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .expect("handshake");
        let mut received = Vec::new();
        while let Ok(Some(Ok(message))) = tokio::time::timeout(IDLE, socket.next()).await {
            received.push(
                serde_json::from_str::<Value>(message.to_text().expect("text frame"))
                    .expect("protocol json"),
            );
        }
        received
    });

    let directory = tempfile::tempdir().expect("temporary cache");
    let cache = CacheWorker::new(&directory.path().join("lyrics.db")).expect("cache worker");
    let service = cache.service();
    let track = track_with("Song", track_artists);
    if let Some(raw_lyrics) = raw_lyrics {
        let (stored, stored_result) = mpsc::channel();
        service.apply_manual(
            track.clone(),
            cached_lyrics_with(raw_lyrics, cached_artists),
            move |result| {
                stored.send(result).expect("apply result");
            },
        );
        stored_result
            .recv_timeout(Duration::from_secs(5))
            .expect("the cache stores the lyrics")
            .expect("the cache accepts the lyrics");
    }

    let (events, receiver) = mpsc::channel();
    let (_hint_sender, hint_receiver) = mpsc::channel();
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.provider_order.clear();
    config.show_romanization = true;
    let mut controller = Controller::new(
        receiver,
        hint_receiver,
        runtime.handle().clone(),
        Rc::new(AmllSender::new(runtime.handle(), &address)),
        service,
        config,
    );
    let handle = controller.handle();
    events
        .send(PlayerWatcherEvent::Connected(player_state(track.clone())))
        .unwrap();

    // Pump the controller the way the GTK tick callback does, long enough for
    // the cached lyrics and the background romanization to be applied.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut offset_applied = false;
    while std::time::Instant::now() < deadline {
        controller.tick();
        if !offset_applied {
            // The offset command needs the track the first tick has connected.
            handle.adjust_track_offset(track_offset_ms);
            offset_applied = true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    runtime
        .block_on(async { tokio::time::timeout(Duration::from_secs(5), player).await })
        .expect("the AMLL player finishes reading")
        .expect("player task")
}

/// The player's volume and playback modes reach the listener, so its own
/// controls render the state of the player FloatLyrics follows.
#[test]
fn publishes_the_player_volume_and_modes_to_the_listener() {
    let received = publish(Some(CACHED_LYRICS), 0);

    assert_eq!(
        last(&received, "volume"),
        serde_json::json!({ "update": "volume", "volume": 0.35 })
    );
    assert_eq!(
        last(&received, "modeChanged"),
        serde_json::json!({ "update": "modeChanged", "repeat": "all", "shuffle": true })
    );
}

/// Returns the TTML document of the last `setLyric` update that carried lyrics.
fn published_lyrics(received: &[Value]) -> String {
    let updates = received
        .iter()
        .filter(|value| value["value"]["update"] == "setLyric")
        .collect::<Vec<_>>();
    let update = updates.last().expect("the listener receives the lyrics");
    assert_eq!(
        update["value"]["format"], "ttml",
        "lyrics reach a listener as a TTML document"
    );
    update["value"]["data"]
        .as_str()
        .expect("the document is a string")
        .to_string()
}

/// Drives the real controller with an [`AmllSender`] and asserts that a
/// listening AMLL player receives the track, the timed lyrics with readings,
/// and the playback progress.
#[test]
fn publishes_controller_state_to_a_listening_player() {
    let received = publish(Some(CACHED_LYRICS), 0);

    assert_eq!(received[0], serde_json::json!({ "type": "initialize" }));
    let music = only(&received, "setMusic");
    assert_eq!(music["musicId"], "/org/mpris/MediaPlayer2/Track/1");
    assert_eq!(music["musicName"], "Song");
    assert_eq!(music["albumName"], "Album");
    assert_eq!(music["albumId"], "");
    assert_eq!(music["duration"], 60_000);
    assert_eq!(
        music["artists"],
        serde_json::json!([{ "id": "", "name": "Artist" }])
    );

    let lyrics = published_lyrics(&received);
    assert!(
        lyrics.contains(r#"<p begin="1.000" end="3.000" ttm:agent="v1" itunes:key="L1">"#),
        "the line carries its own timing: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<span begin="1.000" end="1.200">안</span>"#),
        "a word carries the timing the provider gave it: {lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<transliteration xml:lang="ja-Latn"><text for="L1"><span begin="1.000" end="1.200">an</span>"#
        ),
        "the reading of a word is written per line and placed by time: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<span begin="1.400" end="1.600">se</span>"#),
        "{lyrics}"
    );

    let progress = last(&received, "progress");
    assert!(
        progress["progress"]
            .as_u64()
            .is_some_and(|value| value >= 1_000),
        "the listener receives the playback position, got {progress}"
    );
    assert_eq!(last(&received, "resumed")["update"], "resumed");
}

/// A listener draws its own progress bar, so it receives the playback clock even
/// while the track has no lyrics to send.
#[test]
fn publishes_progress_without_lyrics() {
    let received = publish(None, 0);

    // `last` fails the test when the listener received no progress at all, which
    // is what happened while progress was derived from a rendered lyric frame.
    let progress = last(&received, "progress");
    assert!(
        progress["progress"]
            .as_u64()
            .is_some_and(|value| value >= 1_000),
        "the listener receives the playback position of the playing track, got {progress}"
    );
}

/// The clock is the player's own position: FloatLyrics' per-track lyric offset
/// calibrates the lyrics a view renders, not the listener's progress bar.
#[test]
fn publishes_the_player_position_rather_than_the_lyric_offset() {
    let received = publish(None, -3_000);

    // The fixture reports 1000 ms and the offset would move that below zero.
    let progress = last(&received, "progress")["progress"]
        .as_u64()
        .expect("the listener receives a playback position");
    assert!(
        progress >= 1_000,
        "the lyric offset must not shift the reported position, got {progress}"
    );
}

/// Line-timed lyrics carry no word timing, so the words the listener receives
/// come from segmentation: each line's duration is shared by its tokens and
/// every token receives the reading that covers it.
#[test]
fn splits_line_timed_lyrics_into_words_with_readings() {
    let received = publish(Some(LINE_TIMED_LYRICS), 0);
    let lyrics = published_lyrics(&received);

    assert_eq!(
        lyrics.matches("<p ").count(),
        3,
        "one paragraph per lyric line: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<p begin="1.000" end="3.000" ttm:agent="v1" itunes:key="L1">"#),
        "{lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<span begin="1.000" end="1.500">안</span><span begin="1.500" end="2.000">녕</span> "#,
        ),
        "a line without word timing is split and timed by weight: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<transliteration xml:lang="ja-Latn"><text for="L1"><span begin="1.000" end="1.500">an</span><span begin="1.500" end="2.000">nyeong</span><span begin="2.000" end="2.500">se</span><span begin="2.500" end="3.000">gye</span></text>"#),
        "the readings are timed with the words they belong to: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<span begin="3.000" end="3.500">你</span><span begin="3.500" end="4.000">好</span><span begin="4.000" end="4.500">世</span><span begin="4.500" end="5.000">界</span>"#),
        "Chinese words keep their own readings: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<span begin="3.000" end="3.500">nǐ</span><span begin="3.500" end="4.000">hǎo</span><span begin="4.000" end="4.500">shì</span><span begin="4.500" end="5.000">jiè</span>"#),
        "{lyrics}"
    );
    assert!(
        lyrics.contains(r#"<p begin="5.000" end="1:00.000" ttm:agent="v1" itunes:key="L3">"#),
        "the track duration bounds the last line: {lyrics}"
    );
}

/// Japanese lyrics reach the listener with a reading under every word and the
/// kana of a word written above it, which is what a player draws as furigana.
#[test]
fn splits_japanese_line_timed_lyrics_into_words_with_readings() {
    // A second line bounds the first one, which is what segmentation splits by.
    let received = publish(Some("[00:01.00]こんにちは世界\n[00:05.00]bye"), 0);
    let lyrics = published_lyrics(&received);

    assert!(
        lyrics.contains(r#"<span begin="1.000" end="1.571">こ</span>"#),
        "{lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<span tts:ruby="container"><span tts:ruby="base">世</span><span tts:ruby="textContainer"><span tts:ruby="text" begin="3.855" end="4.426">せ</span></span></span>"#
        ),
        "the kana of a character is written above it: {lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<span tts:ruby="container"><span tts:ruby="base">界</span><span tts:ruby="textContainer"><span tts:ruby="text" begin="4.426" end="5.000">かい</span></span></span>"#
        ),
        "every character of a compound carries its own reading: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<transliteration xml:lang="ja-Latn"><text for="L1"><span begin="1.000" end="1.571">ko</span><span begin="1.571" end="2.142">n</span>"#),
        "the reading of a word is written per line and placed by time: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<span begin="3.284" end="3.855">wa</span>"#),
        "は is read as the particle it is: {lyrics}"
    );
}

/// QRC in the shape a duet and a background vocal are transcribed in: joint
/// speaker labels, a bracketed tail on the line it answers, and a bracketed line
/// of its own timed to the line it answers. The trailing section translates it,
/// which is what the echo has to keep when it is folded away.
const DUET_LYRICS: &str = "\
[0,500]Iggy Azalea/Ariana Grande：(0,500)
[1000,1500]Uh-huh, it's Iggy(1000,500)(oh)(1500,1000)
[3000,500]Ariana Grande：(3000,500)
[4000,1500]I got one less problem(4000,1500)
[6000,1000]((6000,100)One (6100,400)less)(6500,500)
[8000,1500]All I got is one less problem(8000,1500)
[floatlyrics:translation]
[00:04.00]我的烦恼少了一个
[00:06.00](少了一个)";

/// A duet, a background vocal, and the provider's own billing reach the listener
/// the way the transcription states them: each part under its own agent, each
/// answer as a bracketed span on the line it belongs to, and the track restated
/// with the performer the player left out.
#[test]
fn publishes_duet_voices_a_background_vocal_and_the_provider_billing() {
    // The real shape of this: the player bills the lead performer alone while
    // the provider that supplied the lyrics lists both of them.
    let received = publish_with(
        Some(DUET_LYRICS),
        &["Ariana Grande"],
        &["Ariana Grande", "Iggy Azalea"],
        0,
    );

    let lyrics = published_lyrics(&received);
    assert_eq!(
        lyrics.matches("<p ").count(),
        3,
        "a folded echo is not a line of its own: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<p begin="1.000" end="2.500" ttm:agent="v2" itunes:key="L1">"#),
        "the opposing part is written under an agent of its own: {lyrics}"
    );
    assert!(
        lyrics.contains(r#"<p begin="4.000" end="7.000" ttm:agent="v1" itunes:key="L2">"#),
        "the lead part keeps the default agent, and its line runs to the end of the echo that answers it: {lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<span ttm:role="x-bg"><span begin="1.500" end="2.500">(oh)</span></span>"#
        ),
        "a bracketed tail is drawn as the background vocal of its line, at the time of its own words: {lyrics}"
    );
    assert!(
        lyrics.contains(
            r#"<span ttm:role="x-bg"><span begin="6.100" end="6.500">(One </span><span begin="6.500" end="7.000">less)</span><span ttm:role="x-translation" xml:lang="zh-CN">少了一个</span></span>"#
        ),
        "the folded echo keeps its own words, its own time, and its own translation \
         without the bracket that marked it: {lyrics}"
    );
    assert!(
        !lyrics.contains("Iggy Azalea"),
        "a speaker label is not sung text: {lyrics}"
    );

    let music = matching(&received, "setMusic");
    assert_eq!(
        music.len(),
        2,
        "the track is restated once the lyrics credit it: {music:?}"
    );
    assert_eq!(
        music[0]["artists"],
        serde_json::json!([{ "id": "", "name": "Ariana Grande" }])
    );
    assert_eq!(
        music[1]["artists"],
        serde_json::json!([
            { "id": "", "name": "Ariana Grande" },
            { "id": "", "name": "Iggy Azalea" }
        ]),
        "the listener is told the featured performer the player omitted"
    );
}

/// Returns the only message carrying `update`.
fn only(received: &[Value], update: &str) -> Value {
    let matches = matching(received, update);
    assert_eq!(matches.len(), 1, "expected one {update} message");
    matches[0].clone()
}

/// Returns the last message carrying `update`.
fn last(received: &[Value], update: &str) -> Value {
    matching(received, update)
        .pop()
        .unwrap_or_else(|| panic!("expected a {update} message"))
}

fn matching(received: &[Value], update: &str) -> Vec<Value> {
    received
        .iter()
        .filter(|value| value["value"]["update"] == update)
        .map(|value| value["value"].clone())
        .collect()
}

fn track_with(title: &str, artists: &[&str]) -> TrackMetadata {
    TrackMetadata {
        title: title.to_string(),
        artists: artists.iter().map(|artist| artist.to_string()).collect(),
        album: Some("Album".to_string()),
        duration_ms: Some(60_000),
        mpris_track_id: Some("/org/mpris/MediaPlayer2/Track/1".to_string()),
        art_url: None,
    }
}

fn cached_lyrics_with(raw_lyrics: &str, artists: &[&str]) -> FetchedLyrics {
    FetchedLyrics {
        provider: LyricsProvider::QqMusic,
        provider_track_id: Some("42".to_string()),
        title: "Song".to_string(),
        artists: artists.iter().map(|artist| artist.to_string()).collect(),
        score: 1.0,
        raw_lyrics: raw_lyrics.to_string(),
    }
}

/// Mirrors the metadata a playing MPRIS player would publish.
fn player_state(track: TrackMetadata) -> PlayerState {
    PlayerState {
        bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        playback_status: PlaybackStatus::Playing,
        position_ms: Some(1_000),
        control: PlayerControl {
            volume: Some(0.35),
            loop_status: Some(LoopStatus::Playlist),
            shuffle: Some(true),
            ..PlayerControl::default()
        },
        track: Some(track),
    }
}
