//! Scarlet: our own YouTube client. One window, a tab bar on top, and one webview per section so each keeps its place
//! and its sign-in: YouTube, YouTube Music, YouTube TV (the big-screen layout), YouTube Studio, and a Downloads page.
//! Sign-in happens on Google's own page inside the webview; Scarlet never sees the password.
//! On top of the official sites: ad skipping, SponsorBlock, a mini player, and a yt-dlp downloader that only lets you
//! save Creative Commons videos or videos you confirm are your own.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{http::Request, Rect, WebContext, WebView, WebViewBuilder};

const BAR_HEIGHT: f64 = 46.0;
const BAR_HTML: &str = include_str!("../assets/bar.html");
const DOWNLOADS_HTML: &str = include_str!("../assets/downloads.html");
const INJECT_JS: &str = include_str!("../assets/inject.js");

/// A TV browser user agent: youtube.com/tv only shows its big-screen (Leanback) layout to TVs.
const TV_AGENT: &str = "Mozilla/5.0 (SMART-TV; Linux; Tizen 6.5) AppleWebKit/537.36 (KHTML, like Gecko) Version/6.5 TV Safari/537.36";

/// The plain webview on macOS/Linux doesn't call itself Safari, so YouTube Music ("not optimized for your browser") and
/// Google sign-in turn it away. Present as desktop Safari there (Windows' WebView2 already says Edge, which is fine).
const DESKTOP_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15";

/// The site sections, in tab order: (id, start page, user agent override).
const SITES: [(&str, &str, Option<&str>); 4] = [
    ("youtube", "https://www.youtube.com/", None),
    ("music", "https://music.youtube.com/", None),
    ("tv", "https://www.youtube.com/tv", Some(TV_AGENT)),
    ("studio", "https://studio.youtube.com/", None),
];

#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    adblock: bool,
    sponsorblock: bool,
    last_tab: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { adblock: true, sponsorblock: true, last_tab: "youtube".into() }
    }
}

fn data_dir() -> PathBuf {
    let d = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Scarlet");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn download_dir() -> PathBuf {
    let d = dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default()).join("Scarlet");
    let _ = std::fs::create_dir_all(&d);
    d
}

impl Settings {
    fn load() -> Self {
        std::fs::read_to_string(data_dir().join("settings.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    fn save(&self) {
        let _ = std::fs::write(data_dir().join("settings.json"), serde_json::to_string_pretty(self).unwrap_or_default());
    }
    fn js(&self) -> String {
        format!("window.__scarlet = {{ adblock: {}, sponsorblock: {} }};", self.adblock, self.sponsorblock)
    }
}

/// Everything that reaches the main loop from webviews or download threads.
enum Ev {
    Bar(String),
    Downloads(String),
    /// JavaScript to run in the Downloads page (results and progress from yt-dlp threads).
    ToDownloads(String),
}

/// yt-dlp: first `-J` to read title, channel and license; Creative Commons videos are fine to save, anything else only
/// when the user confirms it's their own.
fn check_video(url: String, proxy: EventLoopProxy<Ev>) {
    std::thread::spawn(move || {
        let out = Command::new(ytdlp()).args(["-J", "--no-playlist", "--no-warnings", &url]).output();
        let reply = match out {
            Err(_) => serde_json::json!({ "error": "yt-dlp isn't installed. Install it first (macOS: brew install yt-dlp)." }),
            Ok(o) if !o.status.success() => {
                let err = String::from_utf8_lossy(&o.stderr);
                serde_json::json!({ "error": format!("Couldn't read that video: {}", err.lines().last().unwrap_or("unknown error")) })
            }
            Ok(o) => match serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                Err(_) => serde_json::json!({ "error": "yt-dlp gave an answer Scarlet couldn't read." }),
                Ok(v) => {
                    let license = v["license"].as_str().unwrap_or("").to_string();
                    serde_json::json!({
                        "url": url,
                        "title": v["title"].as_str().unwrap_or("?"),
                        "channel": v["channel"].as_str().or(v["uploader"].as_str()).unwrap_or("?"),
                        "license": license,
                        "cc": license.to_lowercase().contains("creative commons"),
                    })
                }
            },
        };
        let _ = proxy.send_event(Ev::ToDownloads(format!("window.onChecked({reply})")));
    });
}

fn start_download(id: u32, url: String, audio: bool, proxy: EventLoopProxy<Ev>) {
    std::thread::spawn(move || {
        let send = |title: &str, status: &str, percent: f64| {
            let j = serde_json::json!({ "id": id, "title": title, "status": status, "percent": percent });
            let _ = proxy.send_event(Ev::ToDownloads(format!("window.job({j})")));
        };
        let template = download_dir().join("%(title)s [%(id)s].%(ext)s");
        let mut args: Vec<String> = vec!["--newline".into(), "--no-playlist".into(), "--no-warnings".into(), "-o".into(), template.to_string_lossy().into()];
        if audio {
            args.extend(["-f".into(), "bestaudio[ext=m4a]/bestaudio".into(), "-x".into(), "--audio-format".into(), "m4a".into()]);
        } else {
            args.extend(["-f".into(), "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/b".into(), "--merge-output-format".into(), "mp4".into()]);
        }
        args.extend(["--print".into(), "before_dl:TITLE %(title)s".into(), "--progress".into(), url.clone()]);
        let mut title = url.clone();
        send(&title, "Starting…", 0.0);
        let child = Command::new(ytdlp()).args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(_) => return send(&title, "yt-dlp isn't installed (macOS: brew install yt-dlp)", 0.0),
        };
        let mut last = 0.0;
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(t) = line.strip_prefix("TITLE ") {
                    title = t.to_string();
                    send(&title, "Downloading…", 0.0);
                } else if line.starts_with("[download]") {
                    if let Some(p) = line.split_whitespace().find(|w| w.ends_with('%')).and_then(|w| w.trim_end_matches('%').parse::<f64>().ok()) {
                        last = p;
                        send(&title, line.trim_start_matches("[download]").trim(), p);
                    }
                } else if line.starts_with("[Merger]") || line.starts_with("[ExtractAudio]") {
                    send(&title, "Finishing up…", last);
                }
            }
        }
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        if ok {
            send(&title, "Done ✓ saved to Downloads/Scarlet", 100.0);
        } else {
            let mut err = String::new();
            if let Some(e) = child.stderr.take() {
                err = BufReader::new(e).lines().map_while(Result::ok).filter(|l| l.contains("ERROR")).last().unwrap_or_default();
            }
            send(&title, &format!("Failed {}", err), last);
        }
    });
}

/// Homebrew's yt-dlp isn't on the PATH of apps opened from Finder, so look in the usual places too.
fn ytdlp() -> String {
    for p in ["/opt/homebrew/bin/yt-dlp", "/usr/local/bin/yt-dlp", "/usr/bin/yt-dlp"] {
        if std::path::Path::new(p).exists() {
            return p.into();
        }
    }
    "yt-dlp".into()
}

fn content_rect(window: &Window) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    Rect {
        position: LogicalPosition::new(0.0, BAR_HEIGHT).into(),
        size: LogicalSize::new(size.width, (size.height - BAR_HEIGHT).max(0.0)).into(),
    }
}

fn bar_rect(window: &Window) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: LogicalSize::new(size.width, BAR_HEIGHT).into() }
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Scarlet")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(760.0, 480.0))
        .build(&event_loop)
        .expect("could not open a window");

    // A real menu bar: on macOS, copy/paste/select-all in web views and ⌘Q only work through menu items.
    let menu = muda::Menu::new();
    {
        use muda::{PredefinedMenuItem as P, Submenu};
        let about = muda::AboutMetadata { name: Some("Scarlet".into()), version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("YouTube, Music, TV and Studio in one window.".into()), ..Default::default() };
        let app = Submenu::with_items("Scarlet", true, &[&P::about(None, Some(about)), &P::separator(), &P::services(None), &P::separator(),
            &P::hide(None), &P::hide_others(None), &P::show_all(None), &P::separator(), &P::quit(None)]).expect("menu");
        let edit = Submenu::with_items("Edit", true, &[&P::undo(None), &P::redo(None), &P::separator(), &P::cut(None), &P::copy(None),
            &P::paste(None), &P::select_all(None)]).expect("menu");
        let view = Submenu::with_items("View", true, &[&P::fullscreen(None)]).expect("menu");
        let win = Submenu::with_items("Window", true, &[&P::minimize(None), &P::maximize(None), &P::separator(), &P::close_window(None)]).expect("menu");
        menu.append_items(&[&app, &edit, &view, &win]).expect("menu");
        #[cfg(target_os = "macos")]
        menu.init_for_nsapp();
    }

    let mut settings = Settings::load();
    // One shared web context so all YouTube sections share the same Google sign-in, kept between launches.
    let mut context = WebContext::new(Some(data_dir().join("web")));

    let p = proxy.clone();
    let bar = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(BAR_HTML)
        .with_bounds(bar_rect(&window))
        .with_ipc_handler(move |r: Request<String>| {
            let _ = p.send_event(Ev::Bar(r.body().clone()));
        })
        .build_as_child(&window)?;

    let mut pages: HashMap<&'static str, WebView> = HashMap::new();
    for (id, url, agent) in SITES {
        let mut b = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(url)
            .with_bounds(content_rect(&window))
            .with_visible(false)
            .with_initialization_script(&settings.js())
            .with_initialization_script(INJECT_JS)
            .with_back_forward_navigation_gestures(true);
        if let Some(a) = agent {
            b = b.with_user_agent(a);
        } else if !cfg!(windows) {
            b = b.with_user_agent(DESKTOP_AGENT);
        }
        pages.insert(id, b.build_as_child(&window)?);
    }
    let p = proxy.clone();
    let downloads = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(DOWNLOADS_HTML)
        .with_bounds(content_rect(&window))
        .with_visible(false)
        .with_ipc_handler(move |r: Request<String>| {
            let _ = p.send_event(Ev::Downloads(r.body().clone()));
        })
        .build_as_child(&window)?;

    let mut current = if settings.last_tab == "downloads" || pages.contains_key(settings.last_tab.as_str()) {
        settings.last_tab.clone()
    } else {
        "youtube".into()
    };
    let mut next_job = 1u32;

    let show = |tab: &str, pages: &HashMap<&'static str, WebView>, downloads: &WebView, bar: &WebView| {
        for (id, w) in pages {
            let _ = w.set_visible(*id == tab);
        }
        let _ = downloads.set_visible(tab == "downloads");
        let _ = bar.evaluate_script(&format!("window.showTab({:?})", tab));
    };
    show(&current, &pages, &downloads, &bar);
    let _ = bar.evaluate_script(&format!("window.showSettings({})", serde_json::to_string(&settings).unwrap_or_default()));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                let _ = bar.set_bounds(bar_rect(&window));
                for w in pages.values() {
                    let _ = w.set_bounds(content_rect(&window));
                }
                let _ = downloads.set_bounds(content_rect(&window));
            }
            Event::UserEvent(Ev::Bar(msg)) => {
                let page = pages.get(current.as_str());
                if let Some(tab) = msg.strip_prefix("tab:") {
                    if tab == "downloads" || pages.contains_key(tab) {
                        current = tab.to_string();
                        show(&current, &pages, &downloads, &bar);
                        settings.last_tab = current.clone();
                        settings.save();
                    }
                } else if let Some(what) = msg.strip_prefix("toggle:") {
                    match what {
                        "adblock" => settings.adblock = !settings.adblock,
                        "sponsorblock" => settings.sponsorblock = !settings.sponsorblock,
                        _ => {}
                    }
                    settings.save();
                    for w in pages.values() {
                        let _ = w.evaluate_script(&settings.js());
                    }
                    let _ = bar.evaluate_script(&format!("window.showSettings({})", serde_json::to_string(&settings).unwrap_or_default()));
                } else if msg == "back" {
                    if let Some(w) = page {
                        let _ = w.evaluate_script("history.back()");
                    }
                } else if msg == "reload" {
                    if let Some(w) = page {
                        let _ = w.reload();
                    }
                } else if msg == "pip" {
                    if let Some(w) = page {
                        let _ = w.evaluate_script("window.__scarletPip && window.__scarletPip()");
                    }
                } else if msg == "grab" {
                    // Send the video open in the current (or the YouTube) tab to the downloader.
                    let src = page.or_else(|| pages.get("youtube"));
                    let url = src.and_then(|w| w.url().ok()).unwrap_or_default();
                    current = "downloads".into();
                    show(&current, &pages, &downloads, &bar);
                    if url.contains("watch?v=") || url.contains("/shorts/") || url.contains("youtu.be/") {
                        let _ = downloads.evaluate_script(&format!("window.setUrl({:?})", url));
                    }
                }
            }
            Event::UserEvent(Ev::Downloads(msg)) => {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) else { return };
                match v["cmd"].as_str() {
                    Some("ready") => {
                        let _ = downloads.evaluate_script(&format!("window.setFolder({:?})", download_dir().to_string_lossy()));
                    }
                    Some("check") => check_video(v["url"].as_str().unwrap_or("").to_string(), proxy.clone()),
                    Some("download") => {
                        start_download(next_job, v["url"].as_str().unwrap_or("").to_string(), v["format"] == "audio", proxy.clone());
                        next_job += 1;
                    }
                    Some("open_folder") => {
                        let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
                        let _ = Command::new(opener).arg(download_dir()).spawn();
                    }
                    _ => {}
                }
            }
            Event::UserEvent(Ev::ToDownloads(js)) => {
                let _ = downloads.evaluate_script(&js);
            }
            _ => {}
        }
    });
}
