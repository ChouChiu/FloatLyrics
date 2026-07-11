// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightweight, runtime-switchable user-interface translations.
//!
//! The catalogue is compiled into the binary so packaged builds do not depend
//! on an external locale directory.  Widgets subscribe to [`I18n`] and update
//! in place whenever the selected language changes.

use serde::{Deserialize, Serialize};
use std::{cell::Cell, cell::RefCell, env, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en", alias = "en-US", alias = "en-GB")]
    English,
    #[serde(rename = "zh-CN", alias = "zh-cn", alias = "zh-Hans")]
    SimplifiedChinese,
    #[serde(rename = "zh-TW", alias = "zh-tw", alias = "zh-Hant", alias = "zh-HK")]
    TraditionalChinese,
}

impl Default for Language {
    fn default() -> Self {
        Self::detect()
    }
}

impl Language {
    pub const ALL: [Self; 3] = [
        Self::English,
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
            Self::TraditionalChinese => "zh-TW",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
            Self::TraditionalChinese => "繁體中文",
        }
    }

    pub fn detect() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
            .map_or(Self::English, |locale| Self::from_locale(&locale))
    }

    pub fn from_locale(locale: &str) -> Self {
        let locale = locale.to_ascii_lowercase().replace('_', "-");
        if !locale.starts_with("zh") {
            return Self::English;
        }
        if ["hant", "tw", "hk", "mo"]
            .iter()
            .any(|marker| locale.split(['-', '.']).any(|part| part == *marker))
        {
            Self::TraditionalChinese
        } else {
            Self::SimplifiedChinese
        }
    }

    pub const fn text(self, key: Text) -> &'static str {
        match self {
            Self::English => english(key),
            Self::SimplifiedChinese => simplified_chinese(key),
            Self::TraditionalChinese => traditional_chinese(key),
        }
    }

    pub fn detail(self, key: Text, detail: &str) -> String {
        match self {
            Self::English => format!("{}: {detail}", self.text(key)),
            Self::SimplifiedChinese | Self::TraditionalChinese => {
                format!("{}：{detail}", self.text(key))
            }
        }
    }

    pub fn candidates_found(self, count: usize) -> String {
        match self {
            Self::English => match count {
                1 => "Found 1 lyrics candidate".to_string(),
                _ => format!("Found {count} lyrics candidates"),
            },
            Self::SimplifiedChinese => format!("找到 {count} 条候选歌词"),
            Self::TraditionalChinese => format!("找到 {count} 條候選歌詞"),
        }
    }
}

type Listener = Rc<dyn Fn(Language)>;

#[derive(Clone)]
pub struct I18n {
    language: Rc<Cell<Language>>,
    listeners: Rc<RefCell<Vec<Listener>>>,
}

impl I18n {
    pub fn new(language: Language) -> Self {
        Self {
            language: Rc::new(Cell::new(language)),
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn language(&self) -> Language {
        self.language.get()
    }

    pub fn text(&self, key: Text) -> &'static str {
        self.language().text(key)
    }

    pub fn set_language(&self, language: Language) {
        if self.language.replace(language) == language {
            return;
        }
        let listeners = self.listeners.borrow().clone();
        for listener in listeners {
            listener(language);
        }
    }

    pub fn subscribe(&self, listener: impl Fn(Language) + 'static) {
        let listener: Listener = Rc::new(listener);
        listener(self.language());
        self.listeners.borrow_mut().push(listener);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    SettingsWindowTitle,
    ChangesSavedAutomatically,
    Saved,
    SaveFailed,
    General,
    Display,
    LyricsSources,
    GeneralTitle,
    GeneralDescription,
    Language,
    LanguageDescription,
    GlobalOffset,
    GlobalOffsetDescription,
    ShowTranslation,
    ShowTranslationDescription,
    ShowRomanization,
    ShowRomanizationDescription,
    DisplayTitle,
    DisplayDescription,
    PanelWidth,
    PanelWidthDescription,
    BottomMargin,
    BottomMarginDescription,
    BottomPanelHeight,
    BottomPanelHeightDescription,
    BackgroundOpacity,
    BackgroundOpacityDescription,
    SourcesTitle,
    SourcesDescription,
    SearchPriority,
    SearchPriorityDescription,
    QqThenNetEase,
    NetEaseThenQq,
    OpenAbout,
    AboutWindowTitle,
    About,
    Acknowledgements,
    AppSummary,
    Version,
    Copyright,
    LicenseStatement,
    ProjectWebsite,
    AcknowledgementsTitle,
    InspiredByLyricsX,
    VisitLyricsX,
    ManualSearchTooltip,
    OpenSettingsTooltip,
    OpenSpotify,
    SpotifyAttention,
    WaitingForMetadata,
    WaitingForLyrics,
    WaitingForPosition,
    LyricsCacheError,
    SearchingLyrics,
    LyricsParseError,
    CachedLyricsNotSynced,
    LyricsCacheWriteError,
    LyricsSearchFailed,
    DownloadedLyricsNotStored,
    NoLyricsFound,
    SongTitle,
    Artist,
    Search,
    Title,
    SelectCandidatePreview,
    SearchAfterPlayback,
    ApplySelectedLyrics,
    ManualSearchTitle,
    LoadingPreview,
    NoTrackPlaying,
    EnterSongTitle,
    SearchingCandidates,
    SearchingProviders,
    TrackChanged,
    LyricsApplied,
    ApplyFailed,
    NoCandidates,
    SearchFailed,
    LyricsSearchPreviewFailed,
    PreviewReady,
    CandidateUnavailable,
    PreviewLoadFailed,
    LoadingFailed,
    MatchScore,
    OpenSourceTitle,
    OpenSourceDescription,
    CloseTooltip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(Text),
    Detail(Text, String),
}

impl Message {
    pub fn render(&self, language: Language) -> String {
        match self {
            Self::Text(key) => language.text(*key).to_string(),
            Self::Detail(key, detail) => language.detail(*key, detail),
        }
    }

    pub fn key(&self) -> String {
        match self {
            Self::Text(key) => format!("{key:?}"),
            Self::Detail(key, detail) => format!("{key:?}:{detail}"),
        }
    }
}

const fn english(key: Text) -> &'static str {
    match key {
        Text::SettingsWindowTitle => "FloatLyrics Settings",
        Text::ChangesSavedAutomatically => "Changes are saved automatically",
        Text::Saved => "Saved",
        Text::SaveFailed => "Could not save settings",
        Text::General => "General",
        Text::Display => "Display",
        Text::LyricsSources => "Lyrics Sources",
        Text::GeneralTitle => "General",
        Text::GeneralDescription => {
            "Choose a language and adjust lyrics timing and secondary text."
        }
        Text::Language => "Language",
        Text::LanguageDescription => "Changes every open window immediately",
        Text::GlobalOffset => "Global offset",
        Text::GlobalOffsetDescription => "Milliseconds; positive values show lyrics earlier",
        Text::ShowTranslation => "Show translations",
        Text::ShowTranslationDescription => "When supplied by the lyrics source",
        Text::ShowRomanization => "Show romanization",
        Text::ShowRomanizationDescription => "When supplied by the lyrics source",
        Text::DisplayTitle => "Display",
        Text::DisplayDescription => {
            "Control the size, position, and background of the floating panel."
        }
        Text::PanelWidth => "Panel width",
        Text::PanelWidthDescription => "Pixels; long lyrics can expand the panel",
        Text::BottomMargin => "Bottom margin",
        Text::BottomMarginDescription => "Distance from the bottom edge of the screen",
        Text::BottomPanelHeight => "Reserved panel height",
        Text::BottomPanelHeightDescription => "Keeps the lyrics clear of a bottom desktop panel",
        Text::BackgroundOpacity => "Background opacity",
        Text::BackgroundOpacityDescription => "Only affects the floating panel background",
        Text::SourcesTitle => "Lyrics Sources",
        Text::SourcesDescription => {
            "Search online sources in order; changes reload the current track."
        }
        Text::SearchPriority => "Search priority",
        Text::SearchPriorityDescription => "Try the second source when the first has no result",
        Text::QqThenNetEase => "QQ Music → NetEase Cloud Music",
        Text::NetEaseThenQq => "NetEase Cloud Music → QQ Music",
        Text::OpenAbout => "About FloatLyrics",
        Text::AboutWindowTitle => "About FloatLyrics",
        Text::About => "About",
        Text::Acknowledgements => "Acknowledgements",
        Text::AppSummary => "Clean, synchronized floating lyrics for Spotify on Wayland.",
        Text::Version => "Version",
        Text::Copyright => "Copyright © 2026 ChouChiu",
        Text::LicenseStatement => "Licensed under the GNU General Public License v3.0 or later.",
        Text::ProjectWebsite => "Project website",
        Text::AcknowledgementsTitle => "Acknowledgements",
        Text::InspiredByLyricsX => {
            "FloatLyrics is inspired by LyricsX and its thoughtful desktop-lyrics experience."
        }
        Text::VisitLyricsX => "Visit LyricsX on GitHub",
        Text::ManualSearchTooltip => "Select lyrics manually",
        Text::OpenSettingsTooltip => "Open settings",
        Text::OpenSpotify => "Open Spotify to start tracking",
        Text::SpotifyAttention => "The Spotify connection needs attention",
        Text::WaitingForMetadata => "Waiting for Spotify metadata",
        Text::WaitingForLyrics => "Waiting for lyrics",
        Text::WaitingForPosition => "Waiting for playback position",
        Text::LyricsCacheError => "Lyrics cache error",
        Text::SearchingLyrics => "Searching for lyrics…",
        Text::LyricsParseError => "Could not parse lyrics",
        Text::CachedLyricsNotSynced => "Cached lyrics are not time-synced",
        Text::LyricsCacheWriteError => "Could not save lyrics to the cache",
        Text::LyricsSearchFailed => "Lyrics search failed",
        Text::DownloadedLyricsNotStored => "Downloaded lyrics were not stored",
        Text::NoLyricsFound => "No lyrics found from the configured sources",
        Text::SongTitle => "Song title",
        Text::Artist => "Artist",
        Text::Search => "Search",
        Text::Title => "Title",
        Text::SelectCandidatePreview => "Select a candidate to preview its lyrics",
        Text::SearchAfterPlayback => "Play a song to search and bind lyrics manually",
        Text::ApplySelectedLyrics => "Apply selected lyrics",
        Text::ManualSearchTitle => "Select Lyrics Manually",
        Text::LoadingPreview => "Loading lyrics preview…",
        Text::NoTrackPlaying => "No song is currently playing",
        Text::EnterSongTitle => "Enter a song title",
        Text::SearchingCandidates => "Searching for lyrics candidates…",
        Text::SearchingProviders => "Searching QQ Music and NetEase Cloud Music…",
        Text::TrackChanged => "The song changed; search again",
        Text::LyricsApplied => "Lyrics applied and remembered",
        Text::ApplyFailed => "Could not apply lyrics",
        Text::NoCandidates => "No lyrics candidates found",
        Text::SearchFailed => "Search failed",
        Text::LyricsSearchPreviewFailed => "Could not search for lyrics",
        Text::PreviewReady => "Preview loaded; the selected lyrics can be applied",
        Text::CandidateUnavailable => "This candidate has no usable lyrics",
        Text::PreviewLoadFailed => "Could not load the lyrics preview",
        Text::LoadingFailed => "Loading failed",
        Text::MatchScore => "Match score",
        Text::OpenSourceTitle => "Open Source Libraries",
        Text::OpenSourceDescription => {
            "FloatLyrics is built with these great open source projects."
        }
        Text::CloseTooltip => "Close FloatLyrics",
    }
}

const fn simplified_chinese(key: Text) -> &'static str {
    match key {
        Text::SettingsWindowTitle => "FloatLyrics 设置",
        Text::ChangesSavedAutomatically => "所有更改都会自动保存",
        Text::Saved => "已保存",
        Text::SaveFailed => "保存设置失败",
        Text::General => "通用",
        Text::Display => "显示",
        Text::LyricsSources => "歌词源",
        Text::GeneralTitle => "通用",
        Text::GeneralDescription => "选择界面语言，并调整歌词时间与辅助文本。",
        Text::Language => "语言",
        Text::LanguageDescription => "立即更新所有已打开的窗口",
        Text::GlobalOffset => "全局偏移",
        Text::GlobalOffsetDescription => "毫秒；正数会让歌词更早显示",
        Text::ShowTranslation => "显示翻译",
        Text::ShowTranslationDescription => "歌词源提供翻译时显示",
        Text::ShowRomanization => "显示罗马音",
        Text::ShowRomanizationDescription => "歌词源提供罗马音时显示",
        Text::DisplayTitle => "显示",
        Text::DisplayDescription => "控制桌面歌词面板的尺寸、位置与背景。",
        Text::PanelWidth => "面板宽度",
        Text::PanelWidthDescription => "像素；长歌词仍可自动扩展面板",
        Text::BottomMargin => "底部间距",
        Text::BottomMarginDescription => "歌词面板与屏幕底边的距离",
        Text::BottomPanelHeight => "底栏保留高度",
        Text::BottomPanelHeightDescription => "避免歌词遮挡底部桌面栏",
        Text::BackgroundOpacity => "背景不透明度",
        Text::BackgroundOpacityDescription => "仅影响歌词面板背景",
        Text::SourcesTitle => "歌词源",
        Text::SourcesDescription => "按顺序搜索在线歌词；更改后会重新加载当前曲目。",
        Text::SearchPriority => "搜索优先级",
        Text::SearchPriorityDescription => "第一个歌词源无结果时自动尝试第二个",
        Text::QqThenNetEase => "QQ 音乐 → 网易云音乐",
        Text::NetEaseThenQq => "网易云音乐 → QQ 音乐",
        Text::OpenAbout => "关于 FloatLyrics",
        Text::AboutWindowTitle => "关于 FloatLyrics",
        Text::About => "关于",
        Text::Acknowledgements => "致谢",
        Text::AppSummary => "为 Wayland 上的 Spotify 提供简洁、同步的桌面歌词。",
        Text::Version => "版本",
        Text::Copyright => "版权所有 © 2026 ChouChiu",
        Text::LicenseStatement => "本软件采用 GNU 通用公共许可证第 3 版或更新版本授权。",
        Text::ProjectWebsite => "项目主页",
        Text::AcknowledgementsTitle => "致谢",
        Text::InspiredByLyricsX => "FloatLyrics 的灵感来源于 LyricsX 及其出色的桌面歌词体验。",
        Text::VisitLyricsX => "在 GitHub 上访问 LyricsX",
        Text::ManualSearchTooltip => "手动选择歌词",
        Text::OpenSettingsTooltip => "打开设置",
        Text::OpenSpotify => "打开 Spotify 后即可开始跟踪",
        Text::SpotifyAttention => "Spotify 连接需要处理",
        Text::WaitingForMetadata => "正在等待 Spotify 曲目信息",
        Text::WaitingForLyrics => "正在等待歌词",
        Text::WaitingForPosition => "正在等待播放位置",
        Text::LyricsCacheError => "歌词缓存错误",
        Text::SearchingLyrics => "正在搜索歌词…",
        Text::LyricsParseError => "歌词解析失败",
        Text::CachedLyricsNotSynced => "缓存的歌词没有时间轴",
        Text::LyricsCacheWriteError => "歌词写入缓存失败",
        Text::LyricsSearchFailed => "歌词搜索失败",
        Text::DownloadedLyricsNotStored => "下载的歌词未能保存",
        Text::NoLyricsFound => "配置的歌词源均未找到歌词",
        Text::SongTitle => "歌曲名",
        Text::Artist => "艺术家",
        Text::Search => "搜索",
        Text::Title => "标题",
        Text::SelectCandidatePreview => "选择候选歌词后将在这里预览",
        Text::SearchAfterPlayback => "播放歌曲后可搜索并手动绑定歌词",
        Text::ApplySelectedLyrics => "应用所选歌词",
        Text::ManualSearchTitle => "手动选择歌词",
        Text::LoadingPreview => "正在加载歌词预览…",
        Text::NoTrackPlaying => "当前没有正在播放的歌曲",
        Text::EnterSongTitle => "请输入歌曲标题",
        Text::SearchingCandidates => "正在搜索候选歌词…",
        Text::SearchingProviders => "正在搜索 QQ 音乐和网易云音乐…",
        Text::TrackChanged => "歌曲已经切换，请重新搜索",
        Text::LyricsApplied => "已应用并记住这条歌词",
        Text::ApplyFailed => "应用歌词失败",
        Text::NoCandidates => "没有找到候选歌词",
        Text::SearchFailed => "搜索失败",
        Text::LyricsSearchPreviewFailed => "搜索歌词失败",
        Text::PreviewReady => "预览已加载，可应用所选歌词",
        Text::CandidateUnavailable => "该候选没有可用歌词",
        Text::PreviewLoadFailed => "加载歌词预览失败",
        Text::LoadingFailed => "加载失败",
        Text::MatchScore => "匹配度",
        Text::OpenSourceTitle => "开源库",
        Text::OpenSourceDescription => "FloatLyrics 使用了以下优秀的开源项目。",
        Text::CloseTooltip => "关闭 FloatLyrics",
    }
}

const fn traditional_chinese(key: Text) -> &'static str {
    match key {
        Text::SettingsWindowTitle => "FloatLyrics 設定",
        Text::ChangesSavedAutomatically => "所有變更都會自動儲存",
        Text::Saved => "已儲存",
        Text::SaveFailed => "儲存設定失敗",
        Text::General => "一般",
        Text::Display => "顯示",
        Text::LyricsSources => "歌詞來源",
        Text::GeneralTitle => "一般",
        Text::GeneralDescription => "選擇介面語言，並調整歌詞時間與輔助文字。",
        Text::Language => "語言",
        Text::LanguageDescription => "立即更新所有已開啟的視窗",
        Text::GlobalOffset => "全域偏移",
        Text::GlobalOffsetDescription => "毫秒；正數會讓歌詞更早顯示",
        Text::ShowTranslation => "顯示翻譯",
        Text::ShowTranslationDescription => "歌詞來源提供翻譯時顯示",
        Text::ShowRomanization => "顯示羅馬拼音",
        Text::ShowRomanizationDescription => "歌詞來源提供羅馬拼音時顯示",
        Text::DisplayTitle => "顯示",
        Text::DisplayDescription => "控制桌面歌詞面板的尺寸、位置與背景。",
        Text::PanelWidth => "面板寬度",
        Text::PanelWidthDescription => "像素；較長歌詞仍可自動展開面板",
        Text::BottomMargin => "底部間距",
        Text::BottomMarginDescription => "歌詞面板與螢幕底部的距離",
        Text::BottomPanelHeight => "底欄保留高度",
        Text::BottomPanelHeightDescription => "避免歌詞遮擋底部桌面欄",
        Text::BackgroundOpacity => "背景不透明度",
        Text::BackgroundOpacityDescription => "只影響歌詞面板背景",
        Text::SourcesTitle => "歌詞來源",
        Text::SourcesDescription => "依序搜尋線上歌詞；變更後會重新載入目前曲目。",
        Text::SearchPriority => "搜尋優先順序",
        Text::SearchPriorityDescription => "第一個歌詞來源沒有結果時自動嘗試第二個",
        Text::QqThenNetEase => "QQ 音樂 → 網易雲音樂",
        Text::NetEaseThenQq => "網易雲音樂 → QQ 音樂",
        Text::OpenAbout => "關於 FloatLyrics",
        Text::AboutWindowTitle => "關於 FloatLyrics",
        Text::About => "關於",
        Text::Acknowledgements => "致謝",
        Text::AppSummary => "在 Wayland 上為 Spotify 提供簡潔、同步的桌面歌詞。",
        Text::Version => "版本",
        Text::Copyright => "版權所有 © 2026 ChouChiu",
        Text::LicenseStatement => "本軟體採用 GNU 通用公共授權條款第 3 版或更新版本。",
        Text::ProjectWebsite => "專案網站",
        Text::AcknowledgementsTitle => "致謝",
        Text::InspiredByLyricsX => "FloatLyrics 的靈感來自 LyricsX 及其出色的桌面歌詞體驗。",
        Text::VisitLyricsX => "在 GitHub 上瀏覽 LyricsX",
        Text::ManualSearchTooltip => "手動選擇歌詞",
        Text::OpenSettingsTooltip => "開啟設定",
        Text::OpenSpotify => "開啟 Spotify 後即可開始追蹤",
        Text::SpotifyAttention => "Spotify 連線需要處理",
        Text::WaitingForMetadata => "正在等候 Spotify 曲目資訊",
        Text::WaitingForLyrics => "正在等候歌詞",
        Text::WaitingForPosition => "正在等候播放位置",
        Text::LyricsCacheError => "歌詞快取錯誤",
        Text::SearchingLyrics => "正在搜尋歌詞…",
        Text::LyricsParseError => "歌詞解析失敗",
        Text::CachedLyricsNotSynced => "快取的歌詞沒有時間軸",
        Text::LyricsCacheWriteError => "歌詞寫入快取失敗",
        Text::LyricsSearchFailed => "歌詞搜尋失敗",
        Text::DownloadedLyricsNotStored => "下載的歌詞未能儲存",
        Text::NoLyricsFound => "設定的歌詞來源均未找到歌詞",
        Text::SongTitle => "歌曲名稱",
        Text::Artist => "藝人",
        Text::Search => "搜尋",
        Text::Title => "標題",
        Text::SelectCandidatePreview => "選擇候選歌詞後會在這裡預覽",
        Text::SearchAfterPlayback => "播放歌曲後可搜尋並手動綁定歌詞",
        Text::ApplySelectedLyrics => "套用所選歌詞",
        Text::ManualSearchTitle => "手動選擇歌詞",
        Text::LoadingPreview => "正在載入歌詞預覽…",
        Text::NoTrackPlaying => "目前沒有正在播放的歌曲",
        Text::EnterSongTitle => "請輸入歌曲標題",
        Text::SearchingCandidates => "正在搜尋候選歌詞…",
        Text::SearchingProviders => "正在搜尋 QQ 音樂和網易雲音樂…",
        Text::TrackChanged => "歌曲已經切換，請重新搜尋",
        Text::LyricsApplied => "已套用並記住這份歌詞",
        Text::ApplyFailed => "套用歌詞失敗",
        Text::NoCandidates => "找不到候選歌詞",
        Text::SearchFailed => "搜尋失敗",
        Text::LyricsSearchPreviewFailed => "搜尋歌詞失敗",
        Text::PreviewReady => "預覽已載入，可套用所選歌詞",
        Text::CandidateUnavailable => "此候選項目沒有可用歌詞",
        Text::PreviewLoadFailed => "載入歌詞預覽失敗",
        Text::LoadingFailed => "載入失敗",
        Text::MatchScore => "符合度",
        Text::OpenSourceTitle => "開源庫",
        Text::OpenSourceDescription => "FloatLyrics 使用了以下優秀的開源專案。",
        Text::CloseTooltip => "關閉 FloatLyrics",
    }
}

#[cfg(test)]
#[path = "test/i18n_test.rs"]
mod tests;
