//! Lyrics domain facade.
//!
//! Public exports remain stable while models, parsing, provider search, and
//! playback timeline calculations are implemented independently.

mod model;
mod parsing;
mod search;
mod timeline;

pub use lyrics_helper::{LineInfo, LyricsData, LyricsTypes, generate_string, parse_auto};
pub use model::{
    FetchedLyrics, LyricsProvider, LyricsProviderParseError, TimedLine, TimedSyllable,
};
pub use parsing::{
    combine_lyrics_with_translation, export_lyrics, parse_local_lyrics, timed_lines_from_data,
    timed_lines_from_raw,
};
pub use search::{SearchPlan, search_best_lyrics};
pub use timeline::{active_line_index, line_index_at_or_before};

#[cfg(test)]
mod tests {
    use super::search::lyrics_helper_metadata;
    use super::*;

    fn line(start_ms: u64, end_ms: Option<u64>, text: &str) -> TimedLine {
        TimedLine {
            start_ms,
            end_ms,
            text: text.to_string(),
            syllables: Vec::new(),
            translation: None,
            romanization: None,
            background: None,
        }
    }

    #[test]
    fn active_line_uses_offset_and_end_time() {
        let lines = vec![
            line(1_000, Some(2_000), "a"),
            line(2_000, Some(3_000), "b"),
            line(4_000, None, "c"),
        ];

        assert_eq!(active_line_index(&lines, 500, 500), Some(0));
        assert_eq!(active_line_index(&lines, 3_500, 0), None);
        assert_eq!(active_line_index(&lines, 4_200, 0), Some(2));
        assert_eq!(active_line_index(&lines, 100, -500), None);
    }

    #[test]
    fn line_index_at_or_before_holds_previous_line_during_gap() {
        let lines = vec![
            line(1_000, Some(2_000), "a"),
            line(2_000, Some(3_000), "b"),
            line(4_000, None, "c"),
        ];

        assert_eq!(active_line_index(&lines, 3_500, 0), None);
        assert_eq!(line_index_at_or_before(&lines, 3_500, 0), Some(1));
        assert_eq!(line_index_at_or_before(&lines, 100, 0), None);
    }

    #[test]
    fn search_plan_keeps_mvp_provider_order() {
        assert_eq!(
            SearchPlan::default_mvp().providers(),
            &[LyricsProvider::QqMusic, LyricsProvider::NetEase]
        );
    }

    #[test]
    fn parse_and_export_lrc_through_lyrics_helper() {
        let parsed = parse_local_lyrics("[00:01.00]Hello World!").unwrap();
        let exported = export_lyrics(&parsed, LyricsTypes::Lrc).unwrap();

        assert!(exported.contains("Hello World"));
    }

    #[test]
    fn maps_track_metadata_for_lyrics_helper_search() {
        let track = crate::track::TrackMetadata {
            title: "Song".to_string(),
            artists: vec!["Alice".to_string(), "Bob".to_string()],
            album: Some("Album".to_string()),
            duration_ms: Some(123_000),
            mpris_track_id: None,
        };

        let metadata = lyrics_helper_metadata(&track);

        assert_eq!(metadata.title.as_deref(), Some("Song"));
        assert_eq!(metadata.artist.as_deref(), Some("Alice, Bob"));
        assert_eq!(
            metadata.artists.as_deref(),
            Some(&["Alice".to_string(), "Bob".to_string()][..])
        );
        assert_eq!(metadata.album.as_deref(), Some("Album"));
        assert_eq!(metadata.duration_ms, Some(123_000));
    }

    #[test]
    fn converts_lyrics_helper_lines_to_timed_lines() {
        let parsed = parse_local_lyrics("[00:01.00]First\n[00:03.00]Second").unwrap();
        let lines = timed_lines_from_data(&parsed);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 1_000);
        assert_eq!(lines[0].text, "First");
        assert_eq!(lines[1].start_ms, 3_000);
        assert_eq!(active_line_index(&lines, 3_200, 0), Some(1));
    }

    #[test]
    fn combines_translation_lrc_into_timed_lines() {
        let raw = combine_lyrics_with_translation(
            "[00:01.00]Hello\n[00:03.00]World",
            Some("[00:01.00]你好\n[00:03.00]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(lines[1].text, "World");
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn ignores_placeholder_translation_lines() {
        let raw = combine_lyrics_with_translation(
            "[00:01.00]Hello\n[00:03.00]World",
            Some("[00:01.00]//\n[00:03.00]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines[0].translation, None);
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn combines_translation_qrc_into_timed_lines() {
        let raw = combine_lyrics_with_translation(
            "[1000,2000]Hel(1000,500)lo(1500,500)\n[3000,2000]World",
            Some("[1000,2000]你好\n[3000,2000]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 1_000);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(
            lines[0].syllables,
            vec![
                TimedSyllable {
                    start_ms: 1_000,
                    end_ms: 1_500,
                    text: "Hel".to_string(),
                },
                TimedSyllable {
                    start_ms: 1_500,
                    end_ms: 2_000,
                    text: "lo".to_string(),
                },
            ]
        );
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(lines[1].start_ms, 3_000);
        assert_eq!(lines[1].text, "World");
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn filters_intro_title_credit_and_speaker_label_lines() {
        let raw = "\
[0,1800]Señorita(0,600) - (600,200)Shawn(800,400) Mendes(1200,600)
[1800,2000]Lyrics (1800,500)by：(2300,500)Someone(2800,500)
[3800,1200]Both：(3800,1200)
[5000,1600]Camila (5000,500)Cabello：(5500,500)
[6600,2000]Ooh (6600,600)when (7200,400)your (7600,400)lips(8000,600)";
        let lines = timed_lines_from_raw(raw).unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_ms, 6_600);
        assert_eq!(lines[0].text, "Ooh when your lips");
    }

    #[test]
    fn filters_non_lyric_translation_credit_lines() {
        let raw = combine_lyrics_with_translation(
            "[0,2000]Song(0,1000) - Artist(1000,1000)\n[2000,2000]Hello(2000,1000)",
            Some("[00:00.00]QQ音乐享有本翻译作品的著作权\n[00:02.00]你好"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
    }

    #[test]
    fn filters_chinese_standalone_credit_lines() {
        // 词：XXX and 曲：XXX should be filtered (standalone single-char credits)
        let raw = "\
[0,314]BIZNESS(0,157) - XLOV(158,157)
[315,314]词：(315,157)SCORE(473,157)
[630,158]曲：(630,158)QSTNMRKS(788,0)
[789,1000]Dance (789,300)dance(1089,700)";
        let lines = timed_lines_from_raw(raw).unwrap();

        assert_eq!(
            lines.len(),
            1,
            "词：and 曲：credit lines should be filtered"
        );
        assert_eq!(lines[0].start_ms, 789);
        assert_eq!(lines[0].text, "Dance dance");
    }

    #[test]
    fn filters_english_composer_and_arranged_by_lines() {
        // Composer：XXX and Arranged by：XXX should be filtered
        let raw = "\
[0,1060]Song(0,400) - Artist(400,660)
[1060,1060]Composer：(1060,500)Zacharie Raymond(1560,500)
[2120,1060]Arranged (2120,300)by：(2420,500)Charlie Puth(2920,500)
[3180,1000]Hello (3180,400)World(3580,600)";
        let lines = timed_lines_from_raw(raw).unwrap();

        assert_eq!(
            lines.len(),
            1,
            "Composer and Arranged by lines should be filtered"
        );
        assert_eq!(lines[0].text, "Hello World");
    }

    #[test]
    fn search_plan_filters_removed_lrclib_provider() {
        let plan = SearchPlan::new([
            LyricsProvider::LrcLib,
            LyricsProvider::QqMusic,
            LyricsProvider::NetEase,
        ]);

        assert_eq!(
            plan.providers(),
            &[LyricsProvider::QqMusic, LyricsProvider::NetEase]
        );
    }
}
