# Scarlet

Our own YouTube client, inspired by [Red](https://flathub.org/apps/com.ktechpit.red) (Linux) but with more in one window:

| Tab | What it is |
|---|---|
| **YouTube** | the normal site |
| **Music** | YouTube Music |
| **TV** | YouTube's big-screen TV layout (youtube.com/tv, shown with a TV browser identity), nice with a keyboard or gamepad |
| **Studio** | YouTube Studio for your own channel: uploads, analytics, comments |
| **Downloads** | save videos with yt-dlp (see the rules below) |

Each tab is its own web view, so every section keeps its place while you switch. They share one sign-in, kept between
launches. You sign in on Google's own page inside the app: Scarlet never sees your password.

## Extras (top bar)
- **Ad skip** (on by default): hides ad slots and skips or fast-forwards video ads.
- **SponsorBlock** (on by default): skips sponsor, self-promo, "like and subscribe", intro and outro segments, using
  the community database at sponsor.ajay.app. A red pill says when it skipped something.
- **Mini player**: pops the playing video out into a floating picture-in-picture window.
- **⬇ This video**: sends the video you're watching to the Downloads tab.
- **◀** back, **↻** reload.

Your settings are saved in `~/Library/Application Support/Scarlet/settings.json` (macOS).

## Downloads: the rules
YouTube's terms only allow downloading other people's videos through YouTube itself (Premium's offline mode). So
Scarlet downloads only:
- **Creative Commons** videos: it checks the license with yt-dlp.
- **Your own videos**: it compares the video's channel with the channel you're signed in as, which it reads from the
  Studio tab (`studio.youtube.com/channel/UC…`). There's no "trust me" checkbox, and the app (not just the page)
  refuses any link that didn't pass the check.

Files go to `Downloads/Scarlet/`, as MP4 video or M4A audio. Needs [yt-dlp](https://github.com/yt-dlp/yt-dlp) and
ffmpeg (macOS: `brew install yt-dlp ffmpeg`).

## Build and run
Needs Rust. Windows and Linux use the same code (WebView2 on Windows, WebKitGTK on Linux).
```
cargo build --release
./target/release/scarlet
```

Built with [tao](https://crates.io/crates/tao) + [wry](https://crates.io/crates/wry), the window and webview libraries
behind Tauri.

## Mac app
`macos/bundle.sh` builds **Scarlet.app** (with its icon, menu bar and an ad-hoc signature); `macos/bundle.sh --install`
also copies it to /Applications. The icon is drawn by `macos/make-icon.swift`.
