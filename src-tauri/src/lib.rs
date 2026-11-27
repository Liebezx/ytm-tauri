//! Builds the Tauri window, injects page probes, and gates navigation/title messages.
#![recursion_limit = "256"]

mod adblock;
mod controls;
mod feature_bridge;
mod lyrics;
mod notifications;
mod platform;
mod presence;
mod scrobble;
mod scrobble_auth;
mod settings;
mod spotify;
mod spotify_bridge;
mod transfer;
#[cfg(windows)]
mod tray_popup;
mod updates;
mod url_policy;
mod windows_media;

use adblock::AdBlockController;
use controls::AppState;
use notifications::NotificationController;
use presence::{PresenceController, PresenceMessage};
use scrobble::ScrobbleController;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::webview::{NewWindowResponse, WebviewWindowBuilder};
use tauri::{Manager, Theme, Url, WebviewUrl, WindowEvent};
use url_policy::{is_allowed_navigation_url, is_youtube_music_url};

const YOUTUBE_MUSIC_URL: &str = "https://music.youtube.com";
pub const DESKTOP_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const CYBER_TITLEBAR_SCRIPT: &str = include_str!("cyber_titlebar_probe.js");
const AD_BLOCK_SCRIPT: &str = include_str!("adblock_probe.js");
const FEATURE_PROBE_SCRIPT: &str = include_str!("feature_probe.js");
const UI_THEME_SCRIPT: &str = include_str!("ui_theme_probe.js");
const AUDIO_ENGINE_SCRIPT: &str = include_str!("audio_engine_probe.js");
const SYNCED_LYRICS_SCRIPT: &str = include_str!("synced_lyrics_probe.js");
const OUTPUT_DEVICE_SCRIPT: &str = include_str!("output_device_probe.js");
const EQUALIZER_SCRIPT: &str = include_str!("equalizer_probe.js");
const PRECISE_VOLUME_SCRIPT: &str = include_str!("precise_volume_probe.js");
const EXPONENTIAL_VOLUME_SCRIPT: &str = include_str!("exponential_volume_probe.js");
const PLAYBACK_SPEED_SCRIPT: &str = include_str!("playback_speed_probe.js");
const SKIP_DISLIKED_SCRIPT: &str = include_str!("skip_disliked_probe.js");
const NAVIGATION_SCRIPT: &str = include_str!("navigation_probe.js");
const SPONSORBLOCK_SCRIPT: &str = include_str!("sponsorblock_probe.js");
const BLUR_NAV_BAR_SCRIPT: &str = include_str!("blur_nav_bar_probe.js");
const DISABLE_AUTOPLAY_SCRIPT: &str = include_str!("disable_autoplay_probe.js");
const VIDEO_TOGGLE_SCRIPT: &str = include_str!("video_toggle_probe.js");
const AMBIENT_MODE_SCRIPT: &str = include_str!("ambient_mode_probe.js");
const CROSSFADE_SCRIPT: &str = include_str!("crossfade_probe.js");
const SETTINGS_PROBE_SCRIPT: &str = include_str!("settings_probe.js");
const YTM_TRANSFER_SCRIPT: &str = include_str!("ytm_transfer_probe.js");
const SPOTIFY_TRANSFER_SCRIPT: &str = include_str!("spotify_transfer_probe.js");
const PLAYLIST_MANAGER_DATA_SCRIPT: &str = include_str!("playlist_manager_data.js");
const PLAYLIST_MANAGER_SCRIPT: &str = include_str!("playlist_manager_probe.js");
const SHORTCUT_HELP_SCRIPT: &str = include_str!("shortcut_help_probe.js");
const AUTH_RECOVERY_SCRIPT: &str = include_str!("auth_recovery_probe.js");
const TRACK_PROBE_SCRIPT: &str = include_str!("track_probe.js");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NewWindowAction {
    NavigateInMainWebview,
    OpenExternal,
    Deny,
}

// Hidden diagnostics hook: run with YT_MUSIC_ADBLOCK_SELF_TEST=1 and the window
// title should briefly become ADBLOCK_SELF_TEST:PASS when native blocking works.
const AD_BLOCK_SELF_TEST_SCRIPT: &str = r#"
(() => {
  if (!window.__ytMusicTauriAdBlockSelfTest) {
    window.__ytMusicTauriAdBlockSelfTest = true;
    fetch("https://googleads.g.doubleclick.net/pagead/id", { cache: "no-store" })
      .then((response) => {
        document.title = response.status === 204
          ? "ADBLOCK_SELF_TEST:PASS"
          : `ADBLOCK_SELF_TEST:FAIL:${response.status}`;
      })
      .catch((error) => {
        document.title = `ADBLOCK_SELF_TEST:FAIL:${error && error.name ? error.name : "ERROR"}`;
      });
  }
})();
"#;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = platform::register_app_identity();
    platform::allow_foreground_any();
    let settings = settings::load();
    let stored_scrobble = scrobble_auth::load();
    if let Ok(mut value) = settings.lock() {
        value.lastfm_session_key = stored_scrobble.lastfm_session_key.clone();
        value.listenbrainz_token = stored_scrobble.listenbrainz_token.clone();
    }
    let startup_enabled = platform::startup_enabled();
    if settings::snapshot(&settings).launch_at_startup != startup_enabled {
        settings::update(&settings, |value| {
            value.launch_at_startup = startup_enabled
        });
    }
    let initial = settings::snapshot(&settings);
    let presence = PresenceController::new(initial.discord_rpc, initial.spotify_spoof);
    let scrobbler = ScrobbleController::new(settings.clone());
    let notifications = NotificationController::new(settings.clone());
    let adblock = AdBlockController::new(initial.ad_block);
    let presence_for_navigation = presence.clone();
    let presence_for_window = presence.clone();
    let scrobbler_for_navigation = scrobbler.clone();
    let scrobbler_for_window = scrobbler.clone();
    let scrobbler_for_events = scrobbler.clone();
    let notifications_for_navigation = notifications.clone();
    let notifications_for_window = notifications.clone();
    let notifications_for_events = notifications.clone();
    let adblock_for_webview = adblock.clone();
    let spotify = spotify::SpotifyController::new();
    let transfer = transfer::TransferController::new();
    let state = AppState {
        settings,
        presence,
        adblock,
        spotify,
        transfer,
        quitting: Arc::new(AtomicBool::new(false)),
    };
    let state_for_window_events = state.clone();
    let state_for_features = state.clone();
    let state_for_spotify = state.clone();
    let memory_focus_seq = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let memory_focus_seq_events = memory_focus_seq.clone();
    let was_minimized_events = Arc::new(AtomicBool::new(false));
    let start_minimized = std::env::args().any(|argument| argument == "--minimized");

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_notification::init());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                controls::show_main_window(app);
            }));
    }

    let state_flags = tauri_plugin_window_state::StateFlags::all()
        & !tauri_plugin_window_state::StateFlags::DECORATIONS
        & !tauri_plugin_window_state::StateFlags::VISIBLE;

    builder
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["spotify_login", "spotify-session"])
                .with_state_flags(state_flags)
                .build(),
        )
        .on_window_event(move |window, event| match event {
            WindowEvent::CloseRequested { api, .. }
                if window.label() == "main"
                    && settings::snapshot(&state_for_window_events.settings).close_to_tray
                    && !state_for_window_events.quitting.load(Ordering::Relaxed) =>
            {
                api.prevent_close();
                let _ = window.hide();
                memory_focus_seq_events.fetch_add(1, Ordering::Relaxed);
                if let Some(wv_win) = window.app_handle().get_webview_window("main") {
                    let _ = wv_win.with_webview(|wv| platform::set_memory_usage_low(wv, true));
                }
                platform::trim_working_sets_async();
            }
            WindowEvent::Resized(_) if window.label() == "main" => {
                let minimized = window.is_minimized().unwrap_or(false);
                let prev = was_minimized_events.swap(minimized, Ordering::Relaxed);
                if minimized && !prev {
                    memory_focus_seq_events.fetch_add(1, Ordering::Relaxed);
                    if let Some(wv_win) = window.app_handle().get_webview_window("main") {
                        let _ = wv_win.with_webview(|wv| platform::set_memory_usage_low(wv, true));
                    }
                    platform::trim_working_sets_async();
                } else if !minimized && prev {
                    if let Some(wv_win) = window.app_handle().get_webview_window("main") {
                        let _ = wv_win.with_webview(|wv| platform::set_memory_usage_low(wv, false));
                    }
                }
            }
            WindowEvent::Focused(focused) if window.label() == "main" => {
                let seq = memory_focus_seq_events.fetch_add(1, Ordering::Relaxed) + 1;
                if *focused {
                    if let Some(wv_win) = window.app_handle().get_webview_window("main") {
                        let _ = wv_win.with_webview(|wv| platform::set_memory_usage_low(wv, false));
                    }
                } else {
                    let app_handle = window.app_handle().clone();
                    let seq_guard = memory_focus_seq_events.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(10));
                        if seq_guard.load(Ordering::Relaxed) != seq {
                            return;
                        }
                        let app_on_main = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            if seq_guard.load(Ordering::Relaxed) != seq {
                                return;
                            }
                            if let Some(wv_win) = app_on_main.get_webview_window("main") {
                                if !wv_win.is_focused().unwrap_or(false) {
                                    let _ = wv_win
                                        .with_webview(|wv| platform::set_memory_usage_low(wv, true));
                                    platform::trim_working_sets_async();
                                }
                            }
                        });
                    });
                }
            }
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
                if window.label() == "main" =>
            {
                memory_focus_seq_events.fetch_add(1, Ordering::Relaxed);
                state_for_window_events.presence.clear();
                scrobbler_for_events.clear();
                notifications_for_events.clear();
            }
            _ => {}
        })
        .setup(move |app| {
            let music_url = YOUTUBE_MUSIC_URL
                .parse()
                .expect("static YouTube Music URL must be valid");
            let blank_url = "about:blank"
                .parse()
                .expect("static about:blank URL must be valid");
            let app_for_new_window = app.handle().clone();
            let app_for_navigation = app.handle().clone();
            let app_for_landing = app.handle().clone();

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(blank_url))
                .title("YouTube Music")
                .decorations(false)
                .shadow(true)
                .transparent(false)
                .theme(Some(Theme::Dark))
                .background_color(tauri::window::Color(18, 18, 18, 255))
                .inner_size(1280.0, 840.0)
                .min_inner_size(900.0, 620.0)

                .center()
                .devtools(cfg!(debug_assertions))
                .additional_browser_args("--autoplay-policy=no-user-gesture-required --js-flags=--max-old-space-size=384 --renderer-process-limit=1 --num-raster-threads=1 --disk-cache-size=33554432 --media-cache-size=16777216 --disable-features=BackForwardCache,SpareRendererForSitePerProcess,Translate,OptimizationHints,msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-domain-reliability --disable-background-networking")
                .zoom_hotkeys_enabled(false)
                .user_agent(DESKTOP_USER_AGENT)
                .initialization_script(initialization_script(&initial))
                .on_navigation(move |url| {
                    if url_policy::is_youtube_landing_url(url) {
                        if let Some(window) = app_for_landing.get_webview_window("main") {
                            if let Ok(target) = Url::parse(YOUTUBE_MUSIC_URL) {
                                let _ = window.navigate(target);
                            }
                        }
                        return false;
                    }
                    if url_policy::is_auth_recovery_url(url) {
                        static LAST_RECOVERY: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if now.saturating_sub(
                            LAST_RECOVERY.swap(now, std::sync::atomic::Ordering::Relaxed),
                        ) >= 3
                        {
                            if let Some(window) = app_for_navigation.get_webview_window("main") {
                                if let Ok(target) = Url::parse(YOUTUBE_MUSIC_URL) {
                                    let _ = window.navigate(target);
                                }
                            }
                        }
                        return false;
                    }

                    let allowed = is_allowed_navigation_url(url);

                    if !is_youtube_music_url(url) && !url_policy::is_auth_intermediate_url(url) {
                        presence_for_navigation.clear();
                        scrobbler_for_navigation.clear();
                        notifications_for_navigation.clear();
                    }
                    if !allowed && url.scheme() == "https" {
                        platform::open_url(url.as_str());
                    }

                    allowed
                })
                .on_new_window(move |url, _features| {
                    match new_window_action(&url) {
                        NewWindowAction::NavigateInMainWebview => {
                            if let Some(window) = app_for_new_window.get_webview_window("main") {
                                let _ = window.navigate(url);
                            }
                        }
                        NewWindowAction::OpenExternal => {
                            platform::open_url(url.as_str());
                        }
                        NewWindowAction::Deny => {}
                    }

                    NewWindowResponse::Deny
                })
                .on_document_title_changed(move |window, title| {
                    if title.starts_with("YTMCMD:") {
                        let cmd = &title["YTMCMD:".len()..];
                        match cmd {
                            "minimize" => {
                                let _ = window.minimize();
                            }
                            "maximize" => {
                                if window.is_maximized().unwrap_or(false) {
                                    let _ = window.unmaximize();
                                } else {
                                    let _ = window.maximize();
                                }
                            }
                            "close" => {
                                let _ = window.close();
                            }
                            "drag" => {
                                let _ = window.start_dragging();
                            }
                            _ => {}
                        }
                        return;
                    }

                    if feature_bridge::is_feature_title_message(&title) {
                        if window.url().ok().as_ref().is_some_and(is_youtube_music_url) {
                            feature_bridge::handle_title(&window, &title, &state_for_features);
                        }
                        return;
                    }

                    if spotify_bridge::is_spotify_title_message(&title) {
                        if window.url().ok().as_ref().is_some_and(is_youtube_music_url) {
                            spotify_bridge::handle_title(&window, &title, &state_for_spotify);
                        }
                        return;
                    }

                    // track_probe.js sends JSON through document.title so the remote
                    // YouTube page never gets direct access to Tauri commands.
                    if presence::is_presence_title_message(&title) {
                        if let Some(message) = presence::parse_presence_title(&title) {
                            if should_accept_presence_message(window.url().ok().as_ref(), &message)
                            {
                                match message {
                                    PresenceMessage::Track(track) => {
                                        let window_title = track.window_title();
                                        let _ = window.set_title(&window_title);
                                        controls::update_now_playing(
                                            window.app_handle(),
                                            Some(&track),
                                        );
                                        notifications_for_window
                                            .update(window.app_handle(), &track);
                                        windows_media::update(&window, Some(&track));
                                        scrobbler_for_window.update(track.clone());
                                        presence_for_window.update(track);
                                    }
                                    PresenceMessage::Clear => {
                                        let _ = window.set_title("YouTube Music");
                                        controls::update_now_playing(window.app_handle(), None);
                                        notifications_for_window.clear();
                                        windows_media::update(&window, None);
                                        scrobbler_for_window.clear();
                                        presence_for_window.clear();
                                    }
                                }
                            }
                        }
                        return;
                    }

                    if !title.trim().is_empty() {
                        let _ = window.set_title(&title);
                    }
                })
                .build()?;
            if let Err(error) = platform::set_window_app_identity(&window) {
                eprintln!("failed to set Windows window identity: {error}");
            }
            #[cfg(windows)]
            {
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
                    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_THICKFRAME,
                };
                if let Ok(hwnd) = window.hwnd() {
                    unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE);
                        let _ = SetWindowLongW(hwnd, GWL_STYLE, style | WS_THICKFRAME.0 as i32);
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                        );
                    }
                }
            }
            let windows_settings = state.settings.clone();
            let updates_settings = state.settings.clone();
            controls::install(app, state)?;
            windows_media::install(&window, windows_settings);
            let _ = window.set_zoom(initial.zoom.clamp(0.5, 2.0));
            let _ = window.with_webview(move |webview| {
                adblock_for_webview.install(webview);
            });
            let _ = window.with_webview(platform::install_process_recovery);
            window.navigate(music_url)?;
            if start_minimized {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.unminimize();
                platform::force_show_window(&window);
                let _ = window.set_focus();
            }
            updates::check(app.handle(), &updates_settings, updates::CheckMode::Startup);

            let app_for_memory_trim = app.handle().clone();
            std::thread::spawn(move || {
                // Trim cold Chromium/WebView2 initialization pages after hydration.
                std::thread::sleep(std::time::Duration::from_secs(8));
                let app_cold = app_for_memory_trim.clone();
                let _ = app_for_memory_trim.run_on_main_thread(move || {
                    if let Some(wv_win) = app_cold.get_webview_window("main") {
                        let idle = !wv_win.is_visible().unwrap_or(true)
                            || wv_win.is_minimized().unwrap_or(false)
                            || !wv_win.is_focused().unwrap_or(true);
                        if idle {
                            let _ = wv_win
                                .with_webview(|wv| platform::set_memory_usage_low(wv, true));
                        }
                        platform::trim_working_sets_async();
                    }
                });

                std::thread::sleep(std::time::Duration::from_secs(10));
                platform::trim_working_sets();

                loop {
                    std::thread::sleep(std::time::Duration::from_secs(45));
                    let app_tick = app_for_memory_trim.clone();
                    let _ = app_for_memory_trim.run_on_main_thread(move || {
                        if let Some(wv_win) = app_tick.get_webview_window("main") {
                            let idle = !wv_win.is_visible().unwrap_or(true)
                                || wv_win.is_minimized().unwrap_or(false)
                                || !wv_win.is_focused().unwrap_or(false);
                            if idle {
                                let _ = wv_win
                                    .with_webview(|wv| platform::set_memory_usage_low(wv, true));
                                platform::trim_working_sets_async();
                            }
                        }
                    });
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running YouTube Music");
}

fn initialization_script(settings: &settings::Settings) -> String {
    let ad_block = format!(
        "window.__ytMusicTauriAdBlockEnabled = {};",
        settings.ad_block
    );
    let features = format!(
        "window.__ytmFeatureConfig = {};",
        page_feature_config(settings)
    );
    let page_features = format!(
        "{CYBER_TITLEBAR_SCRIPT}\n{FEATURE_PROBE_SCRIPT}\n{UI_THEME_SCRIPT}\n{AUDIO_ENGINE_SCRIPT}\n{SYNCED_LYRICS_SCRIPT}\n{OUTPUT_DEVICE_SCRIPT}\n{EQUALIZER_SCRIPT}\n{PRECISE_VOLUME_SCRIPT}\n{EXPONENTIAL_VOLUME_SCRIPT}\n{PLAYBACK_SPEED_SCRIPT}\n{SKIP_DISLIKED_SCRIPT}\n{NAVIGATION_SCRIPT}\n{SPONSORBLOCK_SCRIPT}\n{BLUR_NAV_BAR_SCRIPT}\n{DISABLE_AUTOPLAY_SCRIPT}\n{VIDEO_TOGGLE_SCRIPT}\n{AMBIENT_MODE_SCRIPT}\n{CROSSFADE_SCRIPT}\n{SETTINGS_PROBE_SCRIPT}\n{YTM_TRANSFER_SCRIPT}\n{SPOTIFY_TRANSFER_SCRIPT}\n{PLAYLIST_MANAGER_DATA_SCRIPT}\n{PLAYLIST_MANAGER_SCRIPT}\n{SHORTCUT_HELP_SCRIPT}"
    );

    if std::env::var_os("YT_MUSIC_ADBLOCK_SELF_TEST").is_some() {
        format!(
            "{AUTH_RECOVERY_SCRIPT}\n{ad_block}\n{features}\n{AD_BLOCK_SCRIPT}\n{page_features}\n{AD_BLOCK_SELF_TEST_SCRIPT}\n{TRACK_PROBE_SCRIPT}"
        )
    } else {
        format!("{AUTH_RECOVERY_SCRIPT}\n{ad_block}\n{features}\n{AD_BLOCK_SCRIPT}\n{page_features}\n{TRACK_PROBE_SCRIPT}")
    }
}

pub fn page_feature_config(settings: &settings::Settings) -> String {
    serde_json::json!({
        "discord_rpc": settings.discord_rpc,
        "ad_block": settings.ad_block,
        "close_to_tray": settings.close_to_tray,
        "launch_at_startup": settings.launch_at_startup,
        "start_minimized": settings.start_minimized,
        "zoom": settings.zoom,
        "synced_lyrics": settings.synced_lyrics,
        "lyrics_precise_timing": settings.lyrics_precise_timing,
        "lyrics_show_inexact": settings.lyrics_show_inexact,
        "lyrics_show_timecodes": settings.lyrics_show_timecodes,
        "lyrics_romanization": settings.lyrics_romanization,
        "lyrics_translation": settings.lyrics_translation,
        "lyrics_auto_sync": settings.lyrics_auto_sync,
        "lyrics_line_effect": settings.lyrics_line_effect,
        "lastfm_scrobbling": settings.lastfm_scrobbling,
        "lastfm_connected": settings.lastfm_session_key.is_some(),
        "listenbrainz_scrobbling": settings.listenbrainz_scrobbling,
        "listenbrainz_connected": settings.listenbrainz_token.is_some(),
        "notifications": settings.notifications,
        "windows_media_controls": settings.windows_media_controls,
        "custom_output_device": settings.custom_output_device,
        "output_device": settings.output_device,
        "equalizer": settings.equalizer,
        "equalizer_preset": settings.equalizer_preset,
        "equalizer_custom_gains": settings.equalizer_custom_gains,
        "precise_volume": settings.precise_volume,
        "exponential_volume": settings.exponential_volume,
        "volume_step": settings.volume_step,
        "navigation_controls": settings.navigation_controls,
        "playback_speed": settings.playback_speed,
        "playback_rate": settings.playback_rate,
        "skip_disliked": settings.skip_disliked,
        "sponsorblock": settings.sponsorblock,
        "blur_nav_bar": settings.blur_nav_bar,
        "disable_autoplay": settings.disable_autoplay,
        "video_toggle": settings.video_toggle,
        "ambient_mode": settings.ambient_mode,
        "crossfade": settings.crossfade,
        "crossfade_seconds": settings.crossfade_seconds,
        "crossfade_curve": settings.crossfade_curve,
        "spotify_spoof": settings.spotify_spoof,
        "app_version": env!("CARGO_PKG_VERSION"),
    })
    .to_string()
}

fn should_accept_presence_message(current_url: Option<&Url>, message: &PresenceMessage) -> bool {
    match message {
        PresenceMessage::Track(_) => current_url.is_some_and(is_youtube_music_url),
        PresenceMessage::Clear => current_url.is_some_and(is_allowed_navigation_url),
    }
}

fn new_window_action(url: &Url) -> NewWindowAction {
    if url.scheme() == "https" && is_allowed_navigation_url(url) {
        NewWindowAction::NavigateInMainWebview
    } else if url.as_str() == "about:blank" {
        NewWindowAction::Deny
    } else {
        NewWindowAction::OpenExternal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_url(value: &str) -> Url {
        Url::parse(value).expect("test URL must parse")
    }

    #[test]
    fn accepts_track_titles_only_from_youtube_music() {
        let title = r#"YTMRPC:{"title":"Song","artist":null,"album":null,"playing":true,"url":null,"cover_url":null,"elapsed_seconds":null,"duration_seconds":null}"#;
        let message = presence::parse_presence_title(title).expect("presence message");
        let music_url = parse_url("https://music.youtube.com/watch?v=x");
        let account_url = parse_url("https://accounts.google.com/signin");

        assert!(should_accept_presence_message(Some(&music_url), &message));
        assert!(!should_accept_presence_message(
            Some(&account_url),
            &message
        ));
        assert!(!should_accept_presence_message(None, &message));
    }

    #[test]
    fn accepts_clear_titles_from_allowed_pages() {
        let message =
            presence::parse_presence_title(r#"YTMRPC:{"type":"clear"}"#).expect("clear message");
        let music_url = parse_url("https://music.youtube.com/watch?v=x");
        let account_url = parse_url("https://accounts.google.com/signin");
        let external_url = parse_url("https://example.com/");

        assert!(should_accept_presence_message(Some(&music_url), &message));
        assert!(should_accept_presence_message(Some(&account_url), &message));
        assert!(!should_accept_presence_message(
            Some(&external_url),
            &message
        ));
        assert!(!should_accept_presence_message(None, &message));
    }

    #[test]
    fn ignores_non_track_titles_for_track_bridge() {
        assert!(presence::parse_presence_title("YouTube Music").is_none());
    }

    #[test]
    fn routes_allowed_new_windows_back_into_main_webview() {
        let music_url = parse_url("https://music.youtube.com/");
        let account_url = parse_url("https://accounts.google.com/signin");
        let external_url = parse_url("https://example.com/");
        let blank_url = parse_url("about:blank");

        assert_eq!(
            new_window_action(&music_url),
            NewWindowAction::NavigateInMainWebview
        );
        assert_eq!(
            new_window_action(&account_url),
            NewWindowAction::NavigateInMainWebview
        );
        assert_eq!(
            new_window_action(&external_url),
            NewWindowAction::OpenExternal
        );
        assert_eq!(new_window_action(&blank_url), NewWindowAction::Deny);
    }

    #[test]
    fn page_config_excludes_native_secrets() {
        let settings = settings::Settings {
            lastfm_session_key: Some("lastfm-secret".to_string()),
            listenbrainz_token: Some("listenbrainz-secret".to_string()),
            ..Default::default()
        };
        let config = page_feature_config(&settings);

        assert!(!config.contains("lastfm-secret"));
        assert!(!config.contains("listenbrainz-secret"));
    }
}
