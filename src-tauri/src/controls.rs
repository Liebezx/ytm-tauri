use crate::{
    adblock::{self, AdBlockController},
    platform,
    presence::{PresenceController, TrackMetadata},
    settings::{self, SharedSettings},
    updates,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{
    menu::{
        CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItem, MenuItemBuilder,
        PredefinedMenuItem, SubmenuBuilder,
    },
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub(crate) const PREVIOUS_ID: &str = "previous";
pub(crate) const PLAY_PAUSE_ID: &str = "play_pause";
pub(crate) const NEXT_ID: &str = "next";
pub(crate) const RELOAD_ID: &str = "reload";
pub(crate) const ZOOM_IN_ID: &str = "zoom_in";
pub(crate) const ZOOM_OUT_ID: &str = "zoom_out";
pub(crate) const ZOOM_RESET_ID: &str = "zoom_reset";
pub(crate) const DISCORD_RPC_ID: &str = "discord_rpc";
pub(crate) const DISCORD_STATUS_ID: &str = "discord_status";
pub(crate) const AD_BLOCK_ID: &str = "ad_block";
pub(crate) const AD_BLOCK_STATUS_ID: &str = "ad_block_status";
pub(crate) const CLOSE_TO_TRAY_ID: &str = "close_to_tray";
pub(crate) const STARTUP_ID: &str = "startup";
pub(crate) const START_MINIMIZED_ID: &str = "start_minimized";
pub(crate) const CLEAR_CACHE_ID: &str = "clear_cache";
pub(crate) const RESET_SESSION_ID: &str = "reset_session";
pub(crate) const CHECK_UPDATES_ID: &str = "check_updates";
pub(crate) const TRAY_SHOW_ID: &str = "tray_show";
pub(crate) const OPEN_SETTINGS_ID: &str = "open_settings";
pub(crate) const TRAY_QUIT_ID: &str = "tray_quit";
pub(crate) const SYNCED_LYRICS_ID: &str = "feature_synced_lyrics";
pub(crate) const LYRICS_TIMECODES_ID: &str = "feature_lyrics_timecodes";
pub(crate) const LYRICS_PRECISE_TIMING_ID: &str = "feature_lyrics_precise_timing";
pub(crate) const LYRICS_ROMANIZATION_ID: &str = "feature_lyrics_romanization";
pub(crate) const LYRICS_TRANSLATION_ID: &str = "feature_lyrics_translation";
pub(crate) const LYRICS_AUTO_SYNC_ID: &str = "feature_lyrics_auto_sync";
pub(crate) const LYRICS_EFFECT_CINEMATIC_ID: &str = "feature_lyrics_effect_cinematic";
pub(crate) const LYRICS_EFFECT_STUDIO_ID: &str = "feature_lyrics_effect_studio";
pub(crate) const LYRICS_EFFECT_LUMINESCENT_ID: &str = "feature_lyrics_effect_luminescent";
pub(crate) const LYRICS_EFFECT_FANCY_ID: &str = "feature_lyrics_effect_fancy";
pub(crate) const LYRICS_EFFECT_SCALE_ID: &str = "feature_lyrics_effect_scale";
pub(crate) const LYRICS_EFFECT_OFFSET_ID: &str = "feature_lyrics_effect_offset";
pub(crate) const LYRICS_EFFECT_FOCUS_ID: &str = "feature_lyrics_effect_focus";
pub(crate) const CYCLE_LYRICS_EFFECT_ID: &str = "feature_cycle_lyrics_effect";
pub(crate) const LASTFM_SCROBBLING_ID: &str = "feature_lastfm_scrobbling";
pub(crate) const LISTENBRAINZ_SCROBBLING_ID: &str = "feature_listenbrainz_scrobbling";
pub(crate) const NOTIFICATIONS_ID: &str = "feature_notifications";
pub(crate) const WINDOWS_MEDIA_CONTROLS_ID: &str = "feature_windows_media_controls";
pub(crate) const CUSTOM_OUTPUT_DEVICE_ID: &str = "feature_custom_output_device";
pub(crate) const EQUALIZER_ID: &str = "feature_equalizer";
pub(crate) const PRECISE_VOLUME_ID: &str = "feature_precise_volume";
pub(crate) const EXPONENTIAL_VOLUME_ID: &str = "feature_exponential_volume";
pub(crate) const NAVIGATION_CONTROLS_ID: &str = "feature_navigation_controls";
pub(crate) const PLAYBACK_SPEED_ID: &str = "feature_playback_speed";
pub(crate) const SKIP_DISLIKED_ID: &str = "feature_skip_disliked";
pub(crate) const SPONSORBLOCK_ID: &str = "feature_sponsorblock";
pub(crate) const BLUR_NAV_BAR_ID: &str = "feature_blur_nav_bar";
pub(crate) const DISABLE_AUTOPLAY_ID: &str = "feature_disable_autoplay";
pub(crate) const VIDEO_TOGGLE_ID: &str = "feature_video_toggle";
pub(crate) const AMBIENT_MODE_ID: &str = "feature_ambient_mode";
pub(crate) const CROSSFADE_ID: &str = "feature_crossfade";
pub(crate) const SPOTIFY_SPOOF_ID: &str = "feature_spotify_spoof";
pub(crate) const DEVTOOLS_ID: &str = "tools_devtools";
const MAX_NOW_PLAYING_CHARS: usize = 72;
const MAX_TRAY_TOOLTIP_CHARS: usize = 120;

#[derive(Clone)]
pub struct AppState {
    pub settings: SharedSettings,
    pub presence: PresenceController,
    pub adblock: AdBlockController,
    pub spotify: crate::spotify::SpotifyController,
    pub transfer: crate::transfer::TransferController,
    pub quitting: Arc<AtomicBool>,
}

struct ShellUi {
    tray: TrayIcon<tauri::Wry>,
    now_playing: MenuItem<tauri::Wry>,
    play_pause: MenuItem<tauri::Wry>,
    discord: CheckMenuItem<tauri::Wry>,
    ad_block: CheckMenuItem<tauri::Wry>,
    close_to_tray: CheckMenuItem<tauri::Wry>,
    startup: CheckMenuItem<tauri::Wry>,
    start_minimized: CheckMenuItem<tauri::Wry>,
    feature_items: HashMap<&'static str, CheckMenuItem<tauri::Wry>>,
    last_track: std::sync::Mutex<Option<TrackMetadata>>,
}

pub fn install(app: &mut App, state: AppState) -> tauri::Result<()> {
    let initial = settings::snapshot(&state.settings);
    let shell = build_tray(app, &initial)?;

    for (shortcut, action) in [
        ("Ctrl+Alt+A", PREVIOUS_ID),
        ("Ctrl+Alt+S", PLAY_PAUSE_ID),
        ("Ctrl+Alt+D", NEXT_ID),
    ] {
        let _ = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    media_action(app, action);
                }
            });
    }

    let state_for_menu = state.clone();
    app.on_menu_event(move |app, event| {
        handle_tray_action(app, event.id().0.as_str(), &state_for_menu);
    });

    app.on_tray_icon_event(|tray, event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        ) {
            #[cfg(windows)]
            crate::tray_popup::hide(tray.app_handle());
            toggle_main_window(tray.app_handle());
        } else if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            #[cfg(windows)]
            crate::tray_popup::toggle(tray.app_handle());
        }
    });

    app.manage(state.clone());
    app.manage(shell);

    #[cfg(windows)]
    crate::tray_popup::install(app.handle().clone(), state);

    Ok(())
}

fn build_tray(app: &App, initial: &settings::Settings) -> tauri::Result<ShellUi> {
    let now_playing = MenuItemBuilder::with_id("now_playing", "Ничего не играет")
        .enabled(false)
        .build(app)?;
    let show = MenuItemBuilder::with_id(TRAY_SHOW_ID, "Показать YouTube Music").build(app)?;

    let previous = MenuItemBuilder::with_id(PREVIOUS_ID, "Предыдущий").build(app)?;
    let play_pause = MenuItemBuilder::with_id(PLAY_PAUSE_ID, "Пауза / Играть").build(app)?;
    let next = MenuItemBuilder::with_id(NEXT_ID, "Следующий").build(app)?;
    let playback = SubmenuBuilder::new(app, "Управление")
        .item(&previous)
        .item(&play_pause)
        .item(&next)
        .build()?;

    let reload = MenuItemBuilder::with_id(RELOAD_ID, "Обновить страницу").build(app)?;
    let zoom_in = MenuItemBuilder::with_id(ZOOM_IN_ID, "Увеличить масштаб").build(app)?;
    let zoom_out = MenuItemBuilder::with_id(ZOOM_OUT_ID, "Уменьшить масштаб").build(app)?;
    let zoom_reset = MenuItemBuilder::with_id(ZOOM_RESET_ID, "Сбросить (100%)").build(app)?;
    let view_separator = PredefinedMenuItem::separator(app)?;
    let view = SubmenuBuilder::new(app, "Вид")
        .item(&reload)
        .item(&view_separator)
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset)
        .build()?;

    let discord = CheckMenuItemBuilder::with_id(DISCORD_RPC_ID, "Discord RPC")
        .checked(initial.discord_rpc)
        .build(app)?;
    let discord_status =
        MenuItemBuilder::with_id(DISCORD_STATUS_ID, "Статус Discord RPC").build(app)?;
    let ad_block = CheckMenuItemBuilder::with_id(AD_BLOCK_ID, "Блокировка рекламы")
        .checked(initial.ad_block)
        .build(app)?;
    let ad_block_status =
        MenuItemBuilder::with_id(AD_BLOCK_STATUS_ID, "Статистика AdBlock").build(app)?;
    let close_to_tray = CheckMenuItemBuilder::with_id(CLOSE_TO_TRAY_ID, "Сворачивать в трей")
        .checked(initial.close_to_tray)
        .build(app)?;
    let startup = CheckMenuItemBuilder::with_id(STARTUP_ID, "Запуск вместе с Windows")
        .checked(initial.launch_at_startup)
        .build(app)?;
    let start_minimized = CheckMenuItemBuilder::with_id(START_MINIMIZED_ID, "Запускать свёрнутым")
        .checked(initial.start_minimized)
        .enabled(initial.launch_at_startup)
        .build(app)?;
    let settings_separator_one = PredefinedMenuItem::separator(app)?;
    let settings_separator_two = PredefinedMenuItem::separator(app)?;
    let settings_menu = SubmenuBuilder::new(app, "Настройки")
        .item(&discord)
        .item(&discord_status)
        .item(&settings_separator_one)
        .item(&ad_block)
        .item(&ad_block_status)
        .item(&settings_separator_two)
        .item(&close_to_tray)
        .item(&startup)
        .item(&start_minimized)
        .build()?;

    let synced_lyrics = feature_item(
        app,
        SYNCED_LYRICS_ID,
        "Синхронный текст песни",
        initial.synced_lyrics,
    )?;
    let lyrics_timecodes = feature_item(
        app,
        LYRICS_TIMECODES_ID,
        "Таймкоды строк",
        initial.lyrics_show_timecodes,
    )?;
    let lyrics_precise = feature_item(
        app,
        LYRICS_PRECISE_TIMING_ID,
        "Точный тайминг (100 мс)",
        initial.lyrics_precise_timing,
    )?;
    let lyrics_romanization = feature_item(
        app,
        LYRICS_ROMANIZATION_ID,
        "Романизация (Romaji)",
        initial.lyrics_romanization,
    )?;
    let lyrics_translation = feature_item(
        app,
        LYRICS_TRANSLATION_ID,
        "Автоперевод текста",
        initial.lyrics_translation,
    )?;
    let lyrics_auto_sync = feature_item(
        app,
        LYRICS_AUTO_SYNC_ID,
        "Автосинхронизация (3 сек)",
        initial.lyrics_auto_sync,
    )?;
    let effect_cinematic = feature_item(
        app,
        LYRICS_EFFECT_CINEMATIC_ID,
        "Кинематограф (глубина и размытие)",
        initial.lyrics_line_effect == "cinematic",
    )?;
    let effect_studio = feature_item(
        app,
        LYRICS_EFFECT_STUDIO_ID,
        "Студия (высокий контраст)",
        initial.lyrics_line_effect == "studio",
    )?;
    let effect_luminescent = feature_item(
        app,
        LYRICS_EFFECT_LUMINESCENT_ID,
        "Люминесцентный (свечение)",
        initial.lyrics_line_effect == "luminescent",
    )?;
    let effect_fancy = feature_item(
        app,
        LYRICS_EFFECT_FANCY_ID,
        "Эффектный (блики и плавность)",
        initial.lyrics_line_effect == "fancy",
    )?;
    let effect_scale = feature_item(
        app,
        LYRICS_EFFECT_SCALE_ID,
        "Масштаб (акцент строки)",
        initial.lyrics_line_effect == "scale",
    )?;
    let effect_offset = feature_item(
        app,
        LYRICS_EFFECT_OFFSET_ID,
        "Сдвиг (отступ активной строки)",
        initial.lyrics_line_effect == "offset",
    )?;
    let effect_focus = feature_item(
        app,
        LYRICS_EFFECT_FOCUS_ID,
        "Фокус (чистая прозрачность)",
        initial.lyrics_line_effect == "focus",
    )?;
    let effect_menu = SubmenuBuilder::new(app, "Эффект анимации строк")
        .item(&effect_cinematic)
        .item(&effect_studio)
        .item(&effect_luminescent)
        .item(&effect_fancy)
        .item(&effect_scale)
        .item(&effect_offset)
        .item(&effect_focus)
        .build()?;

    let lyrics_sep = PredefinedMenuItem::separator(app)?;
    let lyrics_menu = SubmenuBuilder::new(app, "Текст песни")
        .item(&synced_lyrics)
        .item(&lyrics_sep)
        .item(&lyrics_timecodes)
        .item(&lyrics_precise)
        .item(&lyrics_romanization)
        .item(&lyrics_translation)
        .item(&lyrics_auto_sync)
        .item(&effect_menu)
        .build()?;

    let lastfm = feature_item(
        app,
        LASTFM_SCROBBLING_ID,
        "Last.fm скроблинг",
        initial.lastfm_scrobbling,
    )?;
    let listenbrainz = feature_item(
        app,
        LISTENBRAINZ_SCROBBLING_ID,
        "ListenBrainz скроблинг",
        initial.listenbrainz_scrobbling,
    )?;
    let scrobbling = SubmenuBuilder::new(app, "Скроблинг")
        .item(&lastfm)
        .item(&listenbrainz)
        .build()?;

    let notifications = feature_item(
        app,
        NOTIFICATIONS_ID,
        "Уведомления о смене трека",
        initial.notifications,
    )?;
    let windows_media = feature_item(
        app,
        WINDOWS_MEDIA_CONTROLS_ID,
        "Кнопки на панели задач Windows",
        initial.windows_media_controls,
    )?;
    let desktop = SubmenuBuilder::new(app, "Интеграция с Windows")
        .item(&windows_media)
        .item(&notifications)
        .build()?;

    let equalizer = feature_item(
        app,
        EQUALIZER_ID,
        "10-полосный эквалайзер",
        initial.equalizer,
    )?;
    let precise_volume = feature_item(
        app,
        PRECISE_VOLUME_ID,
        "Точная громкость (шаг 1%)",
        initial.precise_volume,
    )?;
    let exponential_volume = feature_item(
        app,
        EXPONENTIAL_VOLUME_ID,
        "Логарифмическая громкость",
        initial.exponential_volume,
    )?;
    let output = feature_item(
        app,
        CUSTOM_OUTPUT_DEVICE_ID,
        "Своё устройство вывода",
        initial.custom_output_device,
    )?;
    let audio_sep = PredefinedMenuItem::separator(app)?;
    let audio = SubmenuBuilder::new(app, "Звук")
        .item(&equalizer)
        .item(&audio_sep)
        .item(&precise_volume)
        .item(&exponential_volume)
        .item(&output)
        .build()?;

    let sponsorblock = feature_item(
        app,
        SPONSORBLOCK_ID,
        "SponsorBlock (пропуск вставок)",
        initial.sponsorblock,
    )?;
    let disable_autoplay = feature_item(
        app,
        DISABLE_AUTOPLAY_ID,
        "Отключить автовоспроизведение",
        initial.disable_autoplay,
    )?;
    let video_toggle = feature_item(
        app,
        VIDEO_TOGGLE_ID,
        "Только аудио (обложка в HQ)",
        initial.video_toggle,
    )?;
    let skip_disliked = feature_item(
        app,
        SKIP_DISLIKED_ID,
        "Пропускать дизлайки",
        initial.skip_disliked,
    )?;
    let playback_speed = feature_item(
        app,
        PLAYBACK_SPEED_ID,
        "Слайдер скорости трека",
        initial.playback_speed,
    )?;
    let crossfade = feature_item(app, CROSSFADE_ID, "Плавный кроссфейд", initial.crossfade)?;
    let smart_playback = SubmenuBuilder::new(app, "Умное воспроизведение")
        .item(&sponsorblock)
        .item(&crossfade)
        .item(&skip_disliked)
        .item(&disable_autoplay)
        .item(&video_toggle)
        .item(&playback_speed)
        .build()?;

    let ambient_mode = feature_item(
        app,
        AMBIENT_MODE_ID,
        "Ambient-подсветка плеера",
        initial.ambient_mode,
    )?;
    let blur_nav_bar = feature_item(
        app,
        BLUR_NAV_BAR_ID,
        "Размытие панелей",
        initial.blur_nav_bar,
    )?;
    let navigation = feature_item(
        app,
        NAVIGATION_CONTROLS_ID,
        "Кнопки Назад / Вперёд в окне",
        initial.navigation_controls,
    )?;
    let spotify_spoof = feature_item(
        app,
        SPOTIFY_SPOOF_ID,
        "Показывать как Spotify в Discord",
        initial.spotify_spoof,
    )?;
    let appearance = SubmenuBuilder::new(app, "Оформление и интерфейс")
        .item(&ambient_mode)
        .item(&blur_nav_bar)
        .item(&navigation)
        .item(&spotify_spoof)
        .build()?;

    let features = SubmenuBuilder::new(app, "Функции")
        .item(&lyrics_menu)
        .item(&smart_playback)
        .item(&audio)
        .item(&appearance)
        .item(&scrobbling)
        .item(&desktop)
        .build()?;

    let feature_items = HashMap::from([
        (SYNCED_LYRICS_ID, synced_lyrics),
        (LYRICS_TIMECODES_ID, lyrics_timecodes),
        (LYRICS_PRECISE_TIMING_ID, lyrics_precise),
        (LYRICS_ROMANIZATION_ID, lyrics_romanization),
        (LYRICS_TRANSLATION_ID, lyrics_translation),
        (LYRICS_AUTO_SYNC_ID, lyrics_auto_sync),
        (LYRICS_EFFECT_CINEMATIC_ID, effect_cinematic),
        (LYRICS_EFFECT_STUDIO_ID, effect_studio),
        (LYRICS_EFFECT_LUMINESCENT_ID, effect_luminescent),
        (LYRICS_EFFECT_FANCY_ID, effect_fancy),
        (LYRICS_EFFECT_SCALE_ID, effect_scale),
        (LYRICS_EFFECT_OFFSET_ID, effect_offset),
        (LYRICS_EFFECT_FOCUS_ID, effect_focus),
        (LASTFM_SCROBBLING_ID, lastfm),
        (LISTENBRAINZ_SCROBBLING_ID, listenbrainz),
        (NOTIFICATIONS_ID, notifications),
        (WINDOWS_MEDIA_CONTROLS_ID, windows_media),
        (CUSTOM_OUTPUT_DEVICE_ID, output),
        (EQUALIZER_ID, equalizer),
        (PRECISE_VOLUME_ID, precise_volume),
        (EXPONENTIAL_VOLUME_ID, exponential_volume),
        (PLAYBACK_SPEED_ID, playback_speed),
        (NAVIGATION_CONTROLS_ID, navigation),
        (SKIP_DISLIKED_ID, skip_disliked),
        (SPONSORBLOCK_ID, sponsorblock),
        (BLUR_NAV_BAR_ID, blur_nav_bar),
        (DISABLE_AUTOPLAY_ID, disable_autoplay),
        (VIDEO_TOGGLE_ID, video_toggle),
        (AMBIENT_MODE_ID, ambient_mode),
        (CROSSFADE_ID, crossfade),
        (SPOTIFY_SPOOF_ID, spotify_spoof),
    ]);

    let clear_cache =
        MenuItemBuilder::with_id(CLEAR_CACHE_ID, "Очистить кэш и перезагрузить").build(app)?;
    let reset_session =
        MenuItemBuilder::with_id(RESET_SESSION_ID, "Сбросить сессию (выйти)").build(app)?;
    let devtools =
        MenuItemBuilder::with_id(DEVTOOLS_ID, "Инструменты разработчика (F12)").build(app)?;
    let check_updates =
        MenuItemBuilder::with_id(CHECK_UPDATES_ID, "Проверить обновления").build(app)?;
    let tools_separator = PredefinedMenuItem::separator(app)?;
    let tools = SubmenuBuilder::new(app, "Система")
        .item(&clear_cache)
        .item(&reset_session)
        .item(&devtools)
        .item(&tools_separator)
        .item(&check_updates)
        .build()?;

    let quit = MenuItemBuilder::with_id(TRAY_QUIT_ID, "Выход").build(app)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let menu = MenuBuilder::new(app)
        .item(&now_playing)
        .item(&show)
        .item(&separator_one)
        .item(&playback)
        .item(&view)
        .item(&settings_menu)
        .item(&features)
        .item(&tools)
        .item(&separator_two)
        .item(&quit)
        .build()?;

    let mut tray = TrayIconBuilder::with_id("main")
        .show_menu_on_left_click(false)
        .tooltip("YouTube Music");
    #[cfg(not(windows))]
    {
        tray = tray.menu(&menu);
    }
    #[cfg(windows)]
    let _ = &menu;
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    let tray = tray.build(app)?;

    Ok(ShellUi {
        tray,
        now_playing,
        play_pause,
        discord,
        ad_block,
        close_to_tray,
        startup,
        start_minimized,
        feature_items,
        last_track: std::sync::Mutex::new(None),
    })
}

fn feature_item(
    app: &App,
    id: &'static str,
    label: &str,
    checked: bool,
) -> tauri::Result<CheckMenuItem<tauri::Wry>> {
    CheckMenuItemBuilder::with_id(id, label)
        .checked(checked)
        .build(app)
}

pub fn handle_tray_action(app: &AppHandle, id: &str, state: &AppState) {
    let shell = app.try_state::<ShellUi>();
    if handle_feature_event(app, id, state, shell.as_deref()) {
        return;
    }

    match id {
        PREVIOUS_ID | PLAY_PAUSE_ID | NEXT_ID => media_action(app, id),
        RELOAD_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.reload();
            }
        }
        ZOOM_IN_ID => set_zoom(app, state, 0.1),
        ZOOM_OUT_ID => set_zoom(app, state, -0.1),
        ZOOM_RESET_ID => reset_zoom(app, state),
        DISCORD_RPC_ID => {
            let mut enabled = false;
            settings::update(&state.settings, |value| {
                value.discord_rpc = !value.discord_rpc;
                enabled = value.discord_rpc;
            });
            if let Some(shell) = shell.as_deref() {
                let _ = shell.discord.set_checked(enabled);
            }
            state.presence.set_enabled(enabled);
            sync_page_features(app, state);
        }
        DISCORD_STATUS_ID => platform::info("Discord RPC", &state.presence.status()),
        AD_BLOCK_ID => {
            let mut enabled = false;
            settings::update(&state.settings, |value| {
                value.ad_block = !value.ad_block;
                enabled = value.ad_block;
            });
            if let Some(shell) = shell.as_deref() {
                let _ = shell.ad_block.set_checked(enabled);
            }
            state.adblock.set_enabled(enabled);
            eval_main(
                app,
                &format!("window.__ytMusicTauriAdBlockEnabled = {enabled}; location.reload();"),
            );
            sync_page_features(app, state);
        }
        AD_BLOCK_STATUS_ID => platform::info(
            "Блокировка рекламы (AdBlock)",
            &format!(
                "Статус: {}\nЗаблокировано запросов за сессию: {}",
                if settings::snapshot(&state.settings).ad_block {
                    "Включена"
                } else {
                    "Выключена"
                },
                state.adblock.blocked_requests()
            ),
        ),
        CLOSE_TO_TRAY_ID => {
            let mut enabled = false;
            settings::update(&state.settings, |value| {
                value.close_to_tray = !value.close_to_tray;
                enabled = value.close_to_tray;
            });
            if let Some(shell) = shell.as_deref() {
                let _ = shell.close_to_tray.set_checked(enabled);
            }
            sync_page_features(app, state);
        }
        STARTUP_ID => set_startup(app, state, shell.as_deref()),
        START_MINIMIZED_ID => set_start_minimized(app, state, shell.as_deref()),
        CLEAR_CACHE_ID => clear_cache(app),
        RESET_SESSION_ID => reset_session(app, state),
        DEVTOOLS_ID => {
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
        }
        CHECK_UPDATES_ID => updates::check(app, &state.settings, updates::CheckMode::Manual),
        TRAY_SHOW_ID => show_main_window(app),
        OPEN_SETTINGS_ID => open_settings(app),
        TRAY_QUIT_ID => {
            state.quitting.store(true, Ordering::Relaxed);
            state.presence.clear();
            app.exit(0);
        }
        _ => {}
    }
}

fn handle_feature_event(
    app: &AppHandle,
    id: &str,
    state: &AppState,
    shell: Option<&ShellUi>,
) -> bool {
    if !matches!(
        id,
        SYNCED_LYRICS_ID
            | LYRICS_TIMECODES_ID
            | LYRICS_PRECISE_TIMING_ID
            | LYRICS_ROMANIZATION_ID
            | LYRICS_TRANSLATION_ID
            | LYRICS_AUTO_SYNC_ID
            | LYRICS_EFFECT_CINEMATIC_ID
            | LYRICS_EFFECT_STUDIO_ID
            | LYRICS_EFFECT_LUMINESCENT_ID
            | LYRICS_EFFECT_FANCY_ID
            | LYRICS_EFFECT_SCALE_ID
            | LYRICS_EFFECT_OFFSET_ID
            | LYRICS_EFFECT_FOCUS_ID
            | CYCLE_LYRICS_EFFECT_ID
            | LASTFM_SCROBBLING_ID
            | LISTENBRAINZ_SCROBBLING_ID
            | NOTIFICATIONS_ID
            | WINDOWS_MEDIA_CONTROLS_ID
            | CUSTOM_OUTPUT_DEVICE_ID
            | EQUALIZER_ID
            | PRECISE_VOLUME_ID
            | EXPONENTIAL_VOLUME_ID
            | NAVIGATION_CONTROLS_ID
            | PLAYBACK_SPEED_ID
            | SKIP_DISLIKED_ID
            | SPONSORBLOCK_ID
            | BLUR_NAV_BAR_ID
            | DISABLE_AUTOPLAY_ID
            | VIDEO_TOGGLE_ID
            | AMBIENT_MODE_ID
            | CROSSFADE_ID
            | SPOTIFY_SPOOF_ID
    ) {
        return false;
    }

    let mut is_now_enabled = false;
    let mut spotify_spoof_changed = None;
    settings::update(&state.settings, |value| match id {
        SYNCED_LYRICS_ID => {
            value.synced_lyrics = !value.synced_lyrics;
            is_now_enabled = value.synced_lyrics;
        }
        LYRICS_TIMECODES_ID => {
            value.lyrics_show_timecodes = !value.lyrics_show_timecodes;
            is_now_enabled = value.lyrics_show_timecodes;
        }
        LYRICS_PRECISE_TIMING_ID => {
            value.lyrics_precise_timing = !value.lyrics_precise_timing;
            is_now_enabled = value.lyrics_precise_timing;
        }
        LYRICS_ROMANIZATION_ID => {
            value.lyrics_romanization = !value.lyrics_romanization;
            is_now_enabled = value.lyrics_romanization;
        }
        LYRICS_TRANSLATION_ID => {
            value.lyrics_translation = !value.lyrics_translation;
            is_now_enabled = value.lyrics_translation;
        }
        LYRICS_AUTO_SYNC_ID => {
            value.lyrics_auto_sync = !value.lyrics_auto_sync;
            is_now_enabled = value.lyrics_auto_sync;
        }
        LYRICS_EFFECT_CINEMATIC_ID => {
            value.lyrics_line_effect = "cinematic".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_STUDIO_ID => {
            value.lyrics_line_effect = "studio".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_LUMINESCENT_ID => {
            value.lyrics_line_effect = "luminescent".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_FANCY_ID => {
            value.lyrics_line_effect = "fancy".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_SCALE_ID => {
            value.lyrics_line_effect = "scale".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_OFFSET_ID => {
            value.lyrics_line_effect = "offset".to_string();
            is_now_enabled = true;
        }
        LYRICS_EFFECT_FOCUS_ID => {
            value.lyrics_line_effect = "focus".to_string();
            is_now_enabled = true;
        }
        CYCLE_LYRICS_EFFECT_ID => {
            let order = [
                "cinematic",
                "studio",
                "luminescent",
                "fancy",
                "scale",
                "offset",
                "focus",
            ];
            let next = order
                .iter()
                .position(|&x| x == value.lyrics_line_effect)
                .map(|i| order[(i + 1) % order.len()])
                .unwrap_or("fancy");
            value.lyrics_line_effect = next.to_string();
            is_now_enabled = true;
        }
        LASTFM_SCROBBLING_ID => {
            value.lastfm_scrobbling = !value.lastfm_scrobbling;
            is_now_enabled = value.lastfm_scrobbling;
        }
        LISTENBRAINZ_SCROBBLING_ID => {
            value.listenbrainz_scrobbling = !value.listenbrainz_scrobbling;
            is_now_enabled = value.listenbrainz_scrobbling;
        }
        NOTIFICATIONS_ID => {
            value.notifications = !value.notifications;
            is_now_enabled = value.notifications;
        }
        WINDOWS_MEDIA_CONTROLS_ID => {
            value.windows_media_controls = !value.windows_media_controls;
            is_now_enabled = value.windows_media_controls;
        }
        CUSTOM_OUTPUT_DEVICE_ID => {
            value.custom_output_device = !value.custom_output_device;
            is_now_enabled = value.custom_output_device;
        }
        EQUALIZER_ID => {
            value.equalizer = !value.equalizer;
            is_now_enabled = value.equalizer;
        }
        PRECISE_VOLUME_ID => {
            value.precise_volume = !value.precise_volume;
            is_now_enabled = value.precise_volume;
        }
        EXPONENTIAL_VOLUME_ID => {
            value.exponential_volume = !value.exponential_volume;
            is_now_enabled = value.exponential_volume;
        }
        NAVIGATION_CONTROLS_ID => {
            value.navigation_controls = !value.navigation_controls;
            is_now_enabled = value.navigation_controls;
        }
        PLAYBACK_SPEED_ID => {
            value.playback_speed = !value.playback_speed;
            is_now_enabled = value.playback_speed;
        }
        SKIP_DISLIKED_ID => {
            value.skip_disliked = !value.skip_disliked;
            is_now_enabled = value.skip_disliked;
        }
        SPONSORBLOCK_ID => {
            value.sponsorblock = !value.sponsorblock;
            is_now_enabled = value.sponsorblock;
        }
        BLUR_NAV_BAR_ID => {
            value.blur_nav_bar = !value.blur_nav_bar;
            is_now_enabled = value.blur_nav_bar;
        }
        DISABLE_AUTOPLAY_ID => {
            value.disable_autoplay = !value.disable_autoplay;
            is_now_enabled = value.disable_autoplay;
        }
        VIDEO_TOGGLE_ID => {
            value.video_toggle = !value.video_toggle;
            is_now_enabled = value.video_toggle;
        }
        AMBIENT_MODE_ID => {
            value.ambient_mode = !value.ambient_mode;
            is_now_enabled = value.ambient_mode;
        }
        CROSSFADE_ID => {
            value.crossfade = !value.crossfade;
            is_now_enabled = value.crossfade;
        }
        SPOTIFY_SPOOF_ID => {
            value.spotify_spoof = !value.spotify_spoof;
            is_now_enabled = value.spotify_spoof;
            spotify_spoof_changed = Some(is_now_enabled);
        }
        _ => {}
    });

    if id.starts_with("feature_lyrics_effect_") || id == CYCLE_LYRICS_EFFECT_ID {
        sync_tray_effects(app, state);
    } else if let Some(item) = shell.and_then(|s| s.feature_items.get(id)) {
        let _ = item.set_checked(is_now_enabled);
    }
    if let Some(spoof) = spotify_spoof_changed {
        state.presence.set_spotify_spoof(spoof);
    }
    if id == WINDOWS_MEDIA_CONTROLS_ID {
        crate::windows_media::refresh(app);
    }
    sync_page_features(app, state);
    true
}

fn sync_page_features(app: &AppHandle, state: &AppState) {
    let config = crate::page_feature_config(&settings::snapshot(&state.settings));
    eval_main(
        app,
        &format!(
            "window.__ytmFeatureConfig = {config}; window.__ytmFeatures?.configure(window.__ytmFeatureConfig);"
        ),
    );
}

pub fn sync_tray_effects(app: &AppHandle, state: &AppState) {
    let Some(shell) = app.try_state::<ShellUi>() else {
        return;
    };
    let current_effect = settings::snapshot(&state.settings).lyrics_line_effect;
    for (id, val) in [
        (LYRICS_EFFECT_CINEMATIC_ID, "cinematic"),
        (LYRICS_EFFECT_STUDIO_ID, "studio"),
        (LYRICS_EFFECT_LUMINESCENT_ID, "luminescent"),
        (LYRICS_EFFECT_FANCY_ID, "fancy"),
        (LYRICS_EFFECT_SCALE_ID, "scale"),
        (LYRICS_EFFECT_OFFSET_ID, "offset"),
        (LYRICS_EFFECT_FOCUS_ID, "focus"),
    ] {
        if let Some(it) = shell.feature_items.get(id) {
            let _ = it.set_checked(current_effect == val);
        }
    }
}

fn set_startup(app: &AppHandle, state: &AppState, shell: Option<&ShellUi>) {
    let snap = settings::snapshot(&state.settings);
    let enabled = !snap.launch_at_startup;
    let minimized = snap.start_minimized;

    match platform::set_startup_enabled(enabled, minimized) {
        Ok(()) => {
            if let Some(shell) = shell {
                let _ = shell.startup.set_checked(enabled);
                let _ = shell.start_minimized.set_enabled(enabled);
            }
            settings::update(&state.settings, |value| {
                value.launch_at_startup = enabled;
                value.start_minimized = minimized;
            });
            sync_page_features(app, state);
        }
        Err(error) => {
            if let Some(shell) = shell {
                let _ = shell.startup.set_checked(!enabled);
            }
            platform::error("Автозапуск с Windows", &error.to_string());
        }
    }
}

fn set_start_minimized(app: &AppHandle, state: &AppState, shell: Option<&ShellUi>) {
    let snap = settings::snapshot(&state.settings);
    let minimized = !snap.start_minimized;
    let enabled = snap.launch_at_startup;

    if let Err(error) = platform::set_startup_enabled(enabled, minimized) {
        if let Some(shell) = shell {
            let _ = shell.start_minimized.set_checked(!minimized);
        }
        platform::error("Автозапуск с Windows", &error.to_string());
        return;
    }

    if let Some(shell) = shell {
        let _ = shell.start_minimized.set_checked(minimized);
    }
    settings::update(&state.settings, |value| value.start_minimized = minimized);
    sync_page_features(app, state);
}

pub fn set_zoom(app: &AppHandle, state: &AppState, delta: f64) {
    let current = settings::snapshot(&state.settings).zoom;
    let next = (((current + delta) * 100.0).round() / 100.0).clamp(0.5, 2.0);
    apply_zoom(app, state, next);
}

pub fn reset_zoom(app: &AppHandle, state: &AppState) {
    apply_zoom(app, state, 1.0);
}

fn apply_zoom(app: &AppHandle, state: &AppState, zoom: f64) {
    let zoom = zoom.clamp(0.5, 2.0);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_zoom(zoom);
        let _ = window.eval(format!(
            "document.documentElement?.style?.setProperty('--ytm-zoom','{zoom:.4}');if(window.__ytmFeatures?.config){{window.__ytmFeatures.config.zoom={zoom:.4};window.dispatchEvent(new CustomEvent('ytm-settings-changed',{{detail:{{...window.__ytmFeatures.config}}}}));}}"
        ));
        settings::update(&state.settings, |value| value.zoom = zoom);
    }
}

pub fn clear_cache(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let reload = window.clone();
        let _ = window.with_webview(move |webview| {
            adblock::clear_cache(webview, move || {
                let _ = reload.reload();
            });
        });
    }
}

pub fn reset_session(app: &AppHandle, state: &AppState) {
    if !platform::confirm(
        "Сброс сессии YouTube Music",
        "Вы выйдете из аккаунта YouTube Music и удалите все локальные данные сайта. Продолжить?",
    ) {
        return;
    }

    state.presence.clear();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.clear_all_browsing_data();
        let _ = window.navigate(
            "https://music.youtube.com"
                .parse()
                .expect("static YouTube Music URL"),
        );
    }
}

fn eval_main(app: &AppHandle, script: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(script);
    }
}

fn media_action(app: &AppHandle, action: &str) {
    let script = match action {
        PREVIOUS_ID => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #previous-button, ytmusic-player-bar #previous-song-button, ytmusic-player-bar .previous-button, ytmusic-player-bar [aria-label^=\"Previous\"], [slot=\"player-bar\"] #previous-button, [slot=\"player-bar\"] .previous-button, [slot=\"player-bar\"] [aria-label^=\"Previous\"], ytmusic-miniplayer [aria-label^=\"Previous\"]'); if (button) button.click(); else { const media = document.querySelector('video, audio'); if (media) media.currentTime = 0; } })();"
        }
        PLAY_PAUSE_ID => {
            "(() => { const media = document.querySelector('video, audio'); if (media) media.paused ? media.play() : media.pause(); else document.querySelector('ytmusic-player-bar #play-pause-button, ytmusic-player-bar .play-pause-button, [slot=\"player-bar\"] #play-pause-button, [slot=\"player-bar\"] .play-pause-button, ytmusic-miniplayer [aria-label^=\"Play\"], ytmusic-miniplayer [aria-label^=\"Pause\"]')?.click(); })();"
        }
        NEXT_ID => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #next-button, ytmusic-player-bar #next-song-button, ytmusic-player-bar .next-button, ytmusic-player-bar [aria-label^=\"Next\"], [slot=\"player-bar\"] #next-button, [slot=\"player-bar\"] .next-button, [slot=\"player-bar\"] [aria-label^=\"Next\"], ytmusic-miniplayer [aria-label^=\"Next\"]'); if (button) button.click(); else { const media = document.querySelector('video, audio'); if (media && Number.isFinite(media.duration)) media.currentTime = media.duration; } })();"
        }
        _ => return,
    };
    eval_main(app, script);
}

pub fn current_track(app: &AppHandle) -> Option<TrackMetadata> {
    app.try_state::<ShellUi>()
        .and_then(|shell| shell.last_track.lock().ok().and_then(|guard| guard.clone()))
}

pub fn is_window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub fn open_settings(app: &AppHandle) {
    show_main_window(app);
    eval_main(app, "window.__ytmOpenSettings?.();");
}

pub fn update_now_playing(app: &AppHandle, track: Option<&TrackMetadata>) {
    let Some(shell) = app.try_state::<ShellUi>() else {
        return;
    };

    if let Ok(mut guard) = shell.last_track.lock() {
        *guard = track.cloned();
    }

    match track {
        Some(track) => {
            let label = now_playing_label(track);
            let tooltip = tray_tooltip(track);
            let _ = shell.now_playing.set_text(label);
            let _ = shell.play_pause.set_text(if track.playing {
                "Пауза"
            } else {
                "Играть"
            });
            let _ = shell.tray.set_tooltip(Some(tooltip));
        }
        None => {
            let _ = shell.now_playing.set_text("Ничего не играет");
            let _ = shell.play_pause.set_text("Пауза / Играть");
            let _ = shell.tray.set_tooltip(Some("YouTube Music"));
        }
    }

    #[cfg(windows)]
    crate::tray_popup::refresh(app);
}

fn now_playing_label(track: &TrackMetadata) -> String {
    let label = match track
        .artist
        .as_deref()
        .filter(|artist| !artist.trim().is_empty())
    {
        Some(artist) => format!("{} - {artist}", track.title),
        None => track.title.clone(),
    };
    truncate_chars(&label, MAX_NOW_PLAYING_CHARS)
}

fn tray_tooltip(track: &TrackMetadata) -> String {
    truncate_chars(
        &format!("YouTube Music\n{}", now_playing_label(track)),
        MAX_TRAY_TOOLTIP_CHARS,
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut shortened: String = value.chars().take(max_chars.saturating_sub(3)).collect();
    shortened.push_str("...");
    shortened
}

pub fn show_main_window(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.with_webview(|wv| crate::platform::set_memory_usage_low(wv, false));
            let _ = window.show();
            let _ = window.unminimize();
            crate::platform::force_show_window(&window);
            let _ = window.set_focus();
        }
    });
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
            let _ = window.hide();
            let _ = window.with_webview(|wv| crate::platform::set_memory_usage_low(wv, true));
            crate::platform::trim_working_sets_async();
        } else {
            show_main_window(app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: Option<&str>, playing: bool) -> TrackMetadata {
        TrackMetadata {
            title: title.to_string(),
            artist: artist.map(str::to_string),
            album: None,
            playing,
            url: None,
            cover_url: None,
            elapsed_seconds: None,
            duration_seconds: None,
        }
    }

    #[test]
    fn now_playing_includes_artist_when_available() {
        assert_eq!(
            now_playing_label(&track("Young Dumb & Broke", Some("Khalid"), true)),
            "Young Dumb & Broke - Khalid"
        );
        assert_eq!(
            now_playing_label(&track("Instrumental", None, false)),
            "Instrumental"
        );
    }

    #[test]
    fn tray_text_is_bounded() {
        let long = "x".repeat(200);
        let track = track(&long, Some("Artist"), true);

        assert!(now_playing_label(&track).chars().count() <= MAX_NOW_PLAYING_CHARS);
        assert!(tray_tooltip(&track).chars().count() <= MAX_TRAY_TOOLTIP_CHARS);
    }
}
