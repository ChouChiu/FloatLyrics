//! Preferences window opened from the command line or a desktop panel.

use gtk::prelude::*;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use crate::{config::AppConfig, lyrics::LyricsProvider};

const SETTINGS_WIDTH: i32 = 680;
const SETTINGS_HEIGHT: i32 = 500;

#[derive(Clone)]
pub(super) struct SettingsWindow {
    window: gtk::ApplicationWindow,
}

impl SettingsWindow {
    pub(super) fn new(
        app: &gtk::Application,
        initial: AppConfig,
        config_file: PathBuf,
        on_saved: impl Fn(AppConfig) + 'static,
    ) -> Self {
        let draft = Rc::new(RefCell::new(initial.clone()));
        let status = gtk::Label::builder()
            .label("所有更改都会自动保存")
            .halign(gtk::Align::Start)
            .css_classes(["settings-status", "dim-label"])
            .build();
        let on_saved: Rc<dyn Fn(AppConfig)> = Rc::new(on_saved);
        let persist: Rc<dyn Fn()> = {
            let draft = Rc::clone(&draft);
            let status = status.clone();
            Rc::new(move || {
                let next = draft.borrow().clone();
                match next.save(&config_file) {
                    Ok(()) => {
                        status.set_label("已保存");
                        status.remove_css_class("error");
                        on_saved(next);
                    }
                    Err(error) => {
                        status.set_label(&format!("保存失败：{error}"));
                        status.add_css_class("error");
                    }
                }
            })
        };

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(160)
            .hexpand(true)
            .vexpand(true)
            .build();

        add_page(
            &stack,
            "lyrics",
            "通用",
            "preferences-system-symbolic",
            &lyrics_page(&initial, &draft, &persist),
        );
        add_page(
            &stack,
            "display",
            "显示",
            "video-display-symbolic",
            &display_page(&initial, &draft, &persist),
        );
        add_page(
            &stack,
            "sources",
            "歌词源",
            "view-list-symbolic",
            &sources_page(&initial, &draft, &persist),
        );

        let switcher = gtk::StackSwitcher::builder()
            .stack(&stack)
            .halign(gtk::Align::Center)
            .build();
        let header = gtk::HeaderBar::builder()
            .title_widget(&switcher)
            .show_title_buttons(true)
            .build();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&stack);
        let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        status_bar.add_css_class("settings-status-bar");
        status_bar.append(&status);
        root.append(&status_bar);

        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("FloatLyrics 设置")
            .default_width(SETTINGS_WIDTH)
            .default_height(SETTINGS_HEIGHT)
            .resizable(true)
            .titlebar(&header)
            .child(&root)
            .hide_on_close(true)
            .build();
        window.add_css_class("settings-window");
        install_css();

        Self { window }
    }

    pub(super) fn present(&self) {
        self.window.present();
    }
}

fn lyrics_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
) -> gtk::ScrolledWindow {
    let offset = gtk::SpinButton::with_range(-10_000.0, 10_000.0, 50.0);
    offset.set_value(config.lyrics.offset_ms as f64);
    offset.set_numeric(true);
    offset.set_tooltip_text(Some("正数会让歌词更早显示，负数会让歌词更晚显示"));
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        offset.connect_value_changed(move |input| {
            draft.borrow_mut().lyrics.offset_ms = input.value_as_int() as i64;
            persist();
        });
    }

    let translation = gtk::Switch::builder()
        .active(config.lyrics.show_translation)
        .valign(gtk::Align::Center)
        .build();
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        translation.connect_active_notify(move |input| {
            draft.borrow_mut().lyrics.show_translation = input.is_active();
            persist();
        });
    }

    let romanization = gtk::Switch::builder()
        .active(config.lyrics.show_romanization)
        .valign(gtk::Align::Center)
        .build();
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        romanization.connect_active_notify(move |input| {
            draft.borrow_mut().lyrics.show_romanization = input.is_active();
            persist();
        });
    }

    page(
        "歌词",
        "调整时间与辅助文本。偏移和显示选项会立即应用到当前歌词。",
        &[setting_card(&[
            setting_row("全局偏移", "毫秒", &offset),
            setting_row("显示翻译", "歌词源提供翻译时显示", &translation),
            setting_row("显示罗马音", "歌词源提供罗马音时显示", &romanization),
        ])],
    )
}

fn display_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
) -> gtk::ScrolledWindow {
    let width = gtk::SpinButton::with_range(320.0, 640.0, 10.0);
    width.set_value(config.window.width as f64);
    width.set_numeric(true);
    connect_window_i32(&width, draft, persist, |config, value| {
        config.window.width = value;
    });

    let margin = gtk::SpinButton::with_range(0.0, 500.0, 4.0);
    margin.set_value(config.window.margin as f64);
    margin.set_numeric(true);
    connect_window_i32(&margin, draft, persist, |config, value| {
        config.window.margin = value;
    });

    let panel_height = gtk::SpinButton::with_range(0.0, 200.0, 2.0);
    panel_height.set_value(config.window.bottom_panel_height as f64);
    panel_height.set_numeric(true);
    connect_window_i32(&panel_height, draft, persist, |config, value| {
        config.window.bottom_panel_height = value;
    });

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.15, 1.0, 0.01);
    opacity.set_value(config.window.opacity.clamp(0.15, 1.0));
    opacity.set_draw_value(true);
    opacity.set_digits(2);
    opacity.set_width_request(190);
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        opacity.connect_value_changed(move |input| {
            draft.borrow_mut().window.opacity = input.value();
            persist();
        });
    }

    page(
        "显示",
        "控制桌面歌词面板的尺寸、位置与背景。",
        &[setting_card(&[
            setting_row("面板宽度", "像素；长歌词仍会自动扩展", &width),
            setting_row("底部间距", "歌词面板与屏幕底边的距离", &margin),
            setting_row(
                "底栏保留高度",
                "避免遮挡底部桌面栏；Noctalia 顶栏布局可设为 0",
                &panel_height,
            ),
            setting_row("背景不透明度", "仅影响歌词面板背景", &opacity),
        ])],
    )
}

fn sources_page(
    config: &AppConfig,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
) -> gtk::ScrolledWindow {
    let source_order =
        gtk::DropDown::from_strings(&["QQ 音乐 → 网易云音乐", "网易云音乐 → QQ 音乐"]);
    let netease_first = config.lyrics.provider_order.first() == Some(&LyricsProvider::NetEase);
    source_order.set_selected(u32::from(netease_first));
    {
        let draft = Rc::clone(draft);
        let persist = Rc::clone(persist);
        source_order.connect_selected_notify(move |input| {
            draft.borrow_mut().lyrics.provider_order = if input.selected() == 1 {
                vec![LyricsProvider::NetEase, LyricsProvider::QqMusic]
            } else {
                vec![LyricsProvider::QqMusic, LyricsProvider::NetEase]
            };
            persist();
        });
    }

    page(
        "歌词源",
        "按顺序搜索在线歌词。更改顺序后会重新加载当前曲目的歌词。",
        &[setting_card(&[setting_row(
            "搜索优先级",
            "第一个歌词源无结果时自动尝试第二个",
            &source_order,
        )])],
    )
}

fn connect_window_i32(
    input: &gtk::SpinButton,
    draft: &Rc<RefCell<AppConfig>>,
    persist: &Rc<dyn Fn()>,
    update: impl Fn(&mut AppConfig, i32) + 'static,
) {
    let draft = Rc::clone(draft);
    let persist = Rc::clone(persist);
    input.connect_value_changed(move |input| {
        update(&mut draft.borrow_mut(), input.value_as_int());
        persist();
    });
}

fn add_page(
    stack: &gtk::Stack,
    name: &str,
    title: &str,
    icon_name: &str,
    child: &impl IsA<gtk::Widget>,
) {
    let page = stack.add_titled(child, Some(name), title);
    page.set_icon_name(icon_name);
}

fn page(title: &str, description: &str, cards: &[gtk::Box]) -> gtk::ScrolledWindow {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("settings-page");

    let title = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .css_classes(["title-1"])
        .build();
    let description = gtk::Label::builder()
        .label(description)
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    content.append(&title);
    content.append(&description);
    for card in cards {
        content.append(card);
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build()
}

fn setting_card(rows: &[gtk::Box]) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-card");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        card.append(row);
    }
    card
}

fn setting_row(title: &str, description: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.append(
        &gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .css_classes(["settings-row-title"])
            .build(),
    );
    labels.append(
        &gtk::Label::builder()
            .label(description)
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    row.add_css_class("settings-row");
    row.append(&labels);
    row.append(control);
    row
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .settings-page {
            padding: 28px 36px 36px;
        }

        .settings-card {
            border: 1px solid alpha(@borders, 0.7);
            border-radius: 12px;
            background: alpha(@theme_base_color, 0.72);
        }

        .settings-row {
            padding: 14px 16px;
        }

        .settings-row-title {
            font-weight: 600;
        }

        .settings-card separator {
            margin-left: 16px;
        }

        .settings-status-bar {
            padding: 8px 16px;
            border-top: 1px solid alpha(@borders, 0.6);
        }

        .settings-status.error {
            color: @error_color;
        }
        "#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
