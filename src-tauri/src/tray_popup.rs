use crate::{
    controls::{
        self, AppState, AD_BLOCK_ID, AD_BLOCK_STATUS_ID, AMBIENT_MODE_ID, BLUR_NAV_BAR_ID,
        CHECK_UPDATES_ID, CLEAR_CACHE_ID, CLOSE_TO_TRAY_ID, CROSSFADE_ID, CUSTOM_OUTPUT_DEVICE_ID,
        CYCLE_LYRICS_EFFECT_ID, DEVTOOLS_ID, DISABLE_AUTOPLAY_ID, DISCORD_RPC_ID,
        DISCORD_STATUS_ID, EQUALIZER_ID, EXPONENTIAL_VOLUME_ID, LASTFM_SCROBBLING_ID,
        LISTENBRAINZ_SCROBBLING_ID, LYRICS_AUTO_SYNC_ID, LYRICS_PRECISE_TIMING_ID,
        LYRICS_ROMANIZATION_ID, LYRICS_TIMECODES_ID, LYRICS_TRANSLATION_ID,
        NAVIGATION_CONTROLS_ID, NEXT_ID, NOTIFICATIONS_ID, OPEN_SETTINGS_ID, PLAYBACK_SPEED_ID,
        PLAY_PAUSE_ID, PRECISE_VOLUME_ID, PREVIOUS_ID, RELOAD_ID, RESET_SESSION_ID,
        SKIP_DISLIKED_ID, SPONSORBLOCK_ID, SPOTIFY_SPOOF_ID, STARTUP_ID, START_MINIMIZED_ID,
        SYNCED_LYRICS_ID, TRAY_QUIT_ID, TRAY_SHOW_ID, VIDEO_TOGGLE_ID,
        WINDOWS_MEDIA_CONTROLS_ID, ZOOM_IN_ID, ZOOM_OUT_ID, ZOOM_RESET_ID,
    },
    settings,
};
use std::{
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicIsize, Ordering},
        Once,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
                    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
                    D2D1_FIGURE_END_OPEN, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F,
                    D2D_SIZE_F,
                },
                D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1GradientStopCollection,
                ID2D1LinearGradientBrush, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE,
                D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP,
                D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_MULTI_THREADED,
                D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_LINE_JOIN_ROUND,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
                D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING,
                DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
                GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject, AC_SRC_ALPHA,
                AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
                MONITORINFO, MONITOR_DEFAULTTONEAREST,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{GetDpiForSystem, GetDpiForWindow},
            Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, GetCursorPos, GetWindowLongPtrW, LoadCursorW,
                RegisterClassW, SetCursor, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
                ShowWindow, UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA,
                HWND_TOPMOST, IDC_ARROW, IDC_HAND, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
                ULW_ALPHA, WM_ACTIVATE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WNDCLASSW, WS_EX_LAYERED,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
};

const POPUP_W: f32 = 372.0;
const POPUP_H: f32 = 518.0;
const SHADOW_PAD: f32 = 14.0;
const CARD_RADIUS: f32 = 16.0;
const WM_MOUSELEAVE: u32 = 0x02A3;

static CLASS_INIT: Once = Once::new();
static POPUP_HWND: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupTab {
    Main,
    Features,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HitAction {
    SelectTab(PopupTab),
    Command(&'static str),
}

#[derive(Clone, Copy, Debug)]
#[allow(non_snake_case)]
struct RectF {
    X: f32,
    Y: f32,
    Width: f32,
    Height: f32,
}

#[derive(Clone, Debug)]
struct HitRegion {
    rect: RectF,
    action: HitAction,
}

struct PopupData {
    app: AppHandle,
    state: AppState,
    active_tab: PopupTab,
    scroll_y: f32,
    max_scroll: f32,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hits: Vec<HitRegion>,
    shown_at: Option<Instant>,
    hidden_at: Option<Instant>,
    tracking_leave: bool,
}

fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub fn install(app: AppHandle, state: AppState) {
    let app_for_thread = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = ensure_window(&app_for_thread, &state);
    });
}

pub fn toggle(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(state) = app_handle.try_state::<AppState>().map(|s| s.inner().clone()) else {
            return;
        };
        let Some(hwnd) = ensure_window(&app_handle, &state) else {
            return;
        };
        unsafe {
            let Some(data) = window_data_mut(hwnd) else {
                return;
            };
            if windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() {
                hide_hwnd(hwnd, data);
                return;
            }
            // Right-clicking the tray icon may trigger WA_INACTIVE milliseconds before WM_RBUTTONUP.
            if data
                .hidden_at
                .is_some_and(|at| at.elapsed() < Duration::from_millis(260))
            {
                data.hidden_at = None;
                return;
            }
            show_hwnd(hwnd, data);
        }
    });
}

pub fn hide(app: &AppHandle) {
    let raw = POPUP_HWND.load(Ordering::Acquire);
    if raw == 0 {
        return;
    }
    let app_handle = app.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let hwnd = HWND(raw as *mut c_void);
        unsafe {
            if let Some(data) = window_data_mut(hwnd) {
                hide_hwnd(hwnd, data);
            }
        }
    });
}

pub fn refresh(app: &AppHandle) {
    let raw = POPUP_HWND.load(Ordering::Acquire);
    if raw == 0 {
        return;
    }
    let app_handle = app.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let hwnd = HWND(raw as *mut c_void);
        unsafe {
            if windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() {
                if let Some(data) = window_data_mut(hwnd) {
                    render_hwnd(hwnd, data);
                }
            }
        }
    });
}

fn ensure_window(app: &AppHandle, state: &AppState) -> Option<HWND> {
    let existing = POPUP_HWND.load(Ordering::Acquire);
    if existing != 0 {
        return Some(HWND(existing as *mut c_void));
    }

    let class_name = HSTRING::from("YtmTrayPopupClass");
    let hinstance = unsafe {
        GetModuleHandleW(None)
            .map(|m| HINSTANCE(m.0))
            .unwrap_or_default()
    };

    CLASS_INIT.call_once(|| unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
    });

    let data = Box::new(PopupData {
        app: app.clone(),
        state: state.clone(),
        active_tab: PopupTab::Main,
        scroll_y: 0.0,
        max_scroll: 0.0,
        hovered: None,
        pressed: None,
        hits: Vec::new(),
        shown_at: None,
        hidden_at: None,
        tracking_leave: false,
    });
    let raw_data = Box::into_raw(data);

    let title = HSTRING::from("YouTube Music Tray Panel");
    let created = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            Some(hinstance),
            Some(raw_data.cast()),
        )
    };

    match created {
        Ok(hwnd) => {
            POPUP_HWND.store(hwnd.0 as isize, Ordering::Release);
            Some(hwnd)
        }
        Err(_) => {
            unsafe {
                drop(Box::from_raw(raw_data));
            }
            None
        }
    }
}

unsafe fn window_data_mut<'a>(hwnd: HWND) -> Option<&'a mut PopupData> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PopupData;
    ptr.as_mut()
}

fn dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe {
        let win_dpi = GetDpiForWindow(hwnd);
        if win_dpi > 0 {
            win_dpi
        } else {
            GetDpiForSystem()
        }
    };
    if dpi == 0 {
        1.0
    } else {
        (dpi as f32 / 96.0).clamp(1.0, 3.0)
    }
}

unsafe fn show_hwnd(hwnd: HWND, data: &mut PopupData) {
    let scale = dpi_scale(hwnd);
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);

    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work = if GetMonitorInfoW(monitor, &mut mi).as_bool() {
        mi.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        }
    };

    let full_w = ((POPUP_W + SHADOW_PAD * 2.0) * scale).round() as i32;
    let max_h = (work.bottom - work.top - 16).max(320);
    let full_h = (((POPUP_H + SHADOW_PAD * 2.0) * scale).round() as i32).min(max_h);

    let (x, y) = compute_popup_origin(cursor, (full_w, full_h), work, (SHADOW_PAD * scale).round() as i32);

    data.hovered = None;
    data.pressed = None;
    data.hidden_at = None;
    data.shown_at = Some(Instant::now());

    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        x,
        y,
        full_w,
        full_h,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    render_hwnd(hwnd, data);
    let _ = SetForegroundWindow(hwnd);
}

unsafe fn hide_hwnd(hwnd: HWND, data: &mut PopupData) {
    if windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() {
        let _ = ShowWindow(hwnd, SW_HIDE);
        data.hovered = None;
        data.pressed = None;
        data.tracking_leave = false;
        data.hidden_at = Some(Instant::now());
    }
}

fn compute_popup_origin(cursor: POINT, win_size: (i32, i32), work: RECT, shadow_pad: i32) -> (i32, i32) {
    let (win_w, win_h) = win_size;
    let margin = 10 - shadow_pad;
    let min_x = work.left + margin;
    let max_x = (work.right - win_w - margin).max(min_x);
    let desired_x = cursor.x - win_w / 2;
    let x = desired_x.clamp(min_x, max_x);

    let min_y = work.top + margin;
    let max_y = (work.bottom - win_h - margin).max(min_y);
    let desired_y = if cursor.y >= work.bottom - 8 {
        work.bottom - win_h - margin
    } else if cursor.y - win_h - 12 >= min_y {
        cursor.y - win_h + shadow_pad - 12
    } else {
        (cursor.y - shadow_pad + 12).min(max_y)
    };
    let y = desired_y.clamp(min_y, max_y);
    (x, y)
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lparam.0
            as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    if msg == WM_NCDESTROY {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PopupData;
        if !ptr.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
        }
        POPUP_HWND.store(0, Ordering::Release);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let Some(data) = window_data_mut(hwnd) else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };

    match msg {
        WM_ACTIVATE => {
            if (wparam.0 & 0xFFFF) == 0 {
                let just_opened = data
                    .shown_at
                    .is_some_and(|at| at.elapsed() < Duration::from_millis(140));
                if !just_opened {
                    hide_hwnd(hwnd, data);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                hide_hwnd(hwnd, data);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            if !data.tracking_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    data.tracking_leave = true;
                }
            }
            let scale = dpi_scale(hwnd);
            let lx = ((lparam.0 & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            let ly = (((lparam.0 >> 16) & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            let next_hover = hit_test(&data.hits, lx, ly);
            let cursor_id = if next_hover.is_some() { IDC_HAND } else { IDC_ARROW };
            if let Ok(cursor) = LoadCursorW(None, cursor_id) {
                SetCursor(Some(cursor));
            }
            if next_hover != data.hovered {
                data.hovered = next_hover;
                render_hwnd(hwnd, data);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            data.tracking_leave = false;
            if data.hovered.is_some() || data.pressed.is_some() {
                data.hovered = None;
                data.pressed = None;
                render_hwnd(hwnd, data);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let scale = dpi_scale(hwnd);
            let lx = ((lparam.0 & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            let ly = (((lparam.0 >> 16) & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            data.pressed = hit_test(&data.hits, lx, ly);
            if data.pressed.is_some() {
                render_hwnd(hwnd, data);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let scale = dpi_scale(hwnd);
            let lx = ((lparam.0 & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            let ly = (((lparam.0 >> 16) & 0xFFFF) as i16) as f32 / scale - SHADOW_PAD;
            let hit_up = hit_test(&data.hits, lx, ly);
            let pressed = data.pressed.take();
            if let (Some(idx), Some(p_idx)) = (hit_up, pressed) {
                if idx == p_idx {
                    if let Some(region) = data.hits.get(idx).cloned() {
                        execute_action(hwnd, data, region.action);
                        return LRESULT(0);
                    }
                }
            }
            render_hwnd(hwnd, data);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if data.active_tab == PopupTab::Features && data.max_scroll > 0.0 {
                let delta = ((wparam.0 >> 16) as i16) as f32 / 120.0;
                let next = clamp_scroll(data.scroll_y - delta * 42.0, data.max_scroll);
                if (next - data.scroll_y).abs() > 0.1 {
                    data.scroll_y = next;
                    let mut pt = POINT::default();
                    if GetCursorPos(&mut pt).is_ok() {
                        let mut rect = RECT::default();
                        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
                        let scale = dpi_scale(hwnd);
                        let lx = (pt.x - rect.left) as f32 / scale - SHADOW_PAD;
                        let ly = (pt.y - rect.top) as f32 / scale - SHADOW_PAD;
                        render_hwnd(hwnd, data);
                        data.hovered = hit_test(&data.hits, lx, ly);
                    }
                    render_hwnd(hwnd, data);
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn hit_test(hits: &[HitRegion], lx: f32, ly: f32) -> Option<usize> {
    hits.iter().rposition(|region| {
        lx >= region.rect.X
            && lx <= region.rect.X + region.rect.Width
            && ly >= region.rect.Y
            && ly <= region.rect.Y + region.rect.Height
    })
}

fn clamp_scroll(value: f32, max_scroll: f32) -> f32 {
    if max_scroll <= 0.0 {
        0.0
    } else {
        value.clamp(0.0, max_scroll)
    }
}

unsafe fn execute_action(hwnd: HWND, data: &mut PopupData, action: HitAction) {
    match action {
        HitAction::SelectTab(tab) => {
            if data.active_tab != tab {
                data.active_tab = tab;
                data.scroll_y = 0.0;
                data.hovered = None;
                data.pressed = None;
            }
            render_hwnd(hwnd, data);
        }
        HitAction::Command(cmd) => {
            let closes_popup = matches!(
                cmd,
                TRAY_SHOW_ID
                    | OPEN_SETTINGS_ID
                    | TRAY_QUIT_ID
                    | DISCORD_STATUS_ID
                    | AD_BLOCK_STATUS_ID
                    | CHECK_UPDATES_ID
                    | RESET_SESSION_ID
                    | DEVTOOLS_ID
            );
            let app = data.app.clone();
            let state = data.state.clone();
            if closes_popup {
                hide_hwnd(hwnd, data);
            }
            controls::handle_tray_action(&app, cmd, &state);
            if !closes_popup {
                render_hwnd(hwnd, data);
            }
        }
    }
}

pub fn lyrics_effect_label(effect: &str) -> &'static str {
    match effect {
        "cinematic" => "Кинематограф",
        "studio" => "Студия",
        "luminescent" => "Люминесцент",
        "fancy" => "Эффектный",
        "scale" => "Масштаб",
        "offset" => "Сдвиг",
        "focus" => "Фокус",
        _ => "Эффектный",
    }
}

// === Direct2D + DirectWrite crisp renderer ===

struct D2dResources {
    d2d: ID2D1Factory,
    dw: IDWriteFactory,
    round_stroke: ID2D1StrokeStyle,
    fonts_scale_bits: u32,
    fonts: Option<FontBank>,
}

thread_local! {
    static D2D_TLS: std::cell::RefCell<Option<D2dResources>> = const { std::cell::RefCell::new(None) };
}

fn with_d2d_resources<R>(scale: f32, f: impl FnOnce(&ID2D1Factory, &ID2D1StrokeStyle, &FontBank) -> R) -> Option<R> {
    D2D_TLS.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let d2d: ID2D1Factory = unsafe {
                D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None)
            }
            .ok()?;
            let dw: IDWriteFactory =
                unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let round_stroke = unsafe { d2d.CreateStrokeStyle(&stroke_props, None) }.ok()?;
            *slot = Some(D2dResources {
                d2d,
                dw,
                round_stroke,
                fonts_scale_bits: 0,
                fonts: None,
            });
        }
        let res = slot.as_mut()?;
        let bits = scale.to_bits();
        if res.fonts.is_none() || res.fonts_scale_bits != bits {
            res.fonts = Some(unsafe { build_font_bank(&res.dw, scale) }.ok()?);
            res.fonts_scale_bits = bits;
        }
        let bank = res.fonts.as_ref()?;
        Some(f(&res.d2d, &res.round_stroke, bank))
    })
}

#[allow(non_upper_case_globals)]
type StringAlignment = DWRITE_TEXT_ALIGNMENT;
#[allow(non_upper_case_globals)]
const StringAlignmentNear: DWRITE_TEXT_ALIGNMENT = DWRITE_TEXT_ALIGNMENT_LEADING;
#[allow(non_upper_case_globals)]
const StringAlignmentCenter: DWRITE_TEXT_ALIGNMENT = DWRITE_TEXT_ALIGNMENT_CENTER;
#[allow(non_upper_case_globals)]
const StringAlignmentFar: DWRITE_TEXT_ALIGNMENT = DWRITE_TEXT_ALIGNMENT_TRAILING;

struct D2dFont {
    near: IDWriteTextFormat,
    center: IDWriteTextFormat,
    far: IDWriteTextFormat,
}

impl D2dFont {
    fn for_align(&self, align: StringAlignment) -> &IDWriteTextFormat {
        if align == StringAlignmentCenter {
            &self.center
        } else if align == StringAlignmentFar {
            &self.far
        } else {
            &self.near
        }
    }
}

struct FontBank {
    title: D2dFont,
    sub: D2dFont,
    small_bold: D2dFont,
    label: D2dFont,
    section: D2dFont,
    tab_active: D2dFont,
}

#[derive(Clone, Copy)]
struct FontRef<'a>(&'a D2dFont);

struct FontSet<'a> {
    title: FontRef<'a>,
    sub: FontRef<'a>,
    small_bold: FontRef<'a>,
    label: FontRef<'a>,
    section: FontRef<'a>,
    tab_active: FontRef<'a>,
}

impl<'a> FontSet<'a> {
    fn from_bank(bank: &'a FontBank) -> Self {
        Self {
            title: FontRef(&bank.title),
            sub: FontRef(&bank.sub),
            small_bold: FontRef(&bank.small_bold),
            label: FontRef(&bank.label),
            section: FontRef(&bank.section),
            tab_active: FontRef(&bank.tab_active),
        }
    }
}

unsafe fn create_dwrite_font(
    dw: &IDWriteFactory,
    size_px: f32,
    weight: DWRITE_FONT_WEIGHT,
) -> windows::core::Result<D2dFont> {
    let family = HSTRING::from("Segoe UI");
    let locale = HSTRING::from("ru-RU");
    let make = |align: DWRITE_TEXT_ALIGNMENT| -> windows::core::Result<IDWriteTextFormat> {
        let fmt = dw.CreateTextFormat(
            &family,
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size_px,
            &locale,
        )?;
        fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        fmt.SetTextAlignment(align)?;
        let sign = dw.CreateEllipsisTrimmingSign(&fmt)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        fmt.SetTrimming(&trimming, &sign)?;
        Ok(fmt)
    };
    Ok(D2dFont {
        near: make(DWRITE_TEXT_ALIGNMENT_LEADING)?,
        center: make(DWRITE_TEXT_ALIGNMENT_CENTER)?,
        far: make(DWRITE_TEXT_ALIGNMENT_TRAILING)?,
    })
}

unsafe fn build_font_bank(dw: &IDWriteFactory, scale: f32) -> windows::core::Result<FontBank> {
    Ok(FontBank {
        title: create_dwrite_font(dw, (13.5 * scale).round().max(11.0), DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
        sub: create_dwrite_font(dw, (11.5 * scale).round().max(10.0), DWRITE_FONT_WEIGHT_REGULAR)?,
        small_bold: create_dwrite_font(dw, (10.5 * scale).round().max(9.0), DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
        label: create_dwrite_font(dw, (12.5 * scale).round().max(10.5), DWRITE_FONT_WEIGHT_REGULAR)?,
        section: create_dwrite_font(dw, (10.8 * scale).round().max(9.5), DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
        tab_active: create_dwrite_font(dw, (12.5 * scale).round().max(10.5), DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
    })
}

struct D2dCanvas<'a> {
    d2d: &'a ID2D1Factory,
    rt: &'a ID2D1RenderTarget,
    stroke_style: &'a ID2D1StrokeStyle,
}

#[derive(Clone, Copy)]
struct CanvasHolder<'a>(&'a D2dCanvas<'a>);

fn color_from_argb(color: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((color >> 16) & 0xFF) as f32 / 255.0,
        g: ((color >> 8) & 0xFF) as f32 / 255.0,
        b: (color & 0xFF) as f32 / 255.0,
        a: ((color >> 24) & 0xFF) as f32 / 255.0,
    }
}

fn d2d_pt(x: f32, y: f32) -> windows_numerics::Vector2 {
    windows_numerics::Vector2 { X: x, Y: y }
}

unsafe fn make_solid_brush(g: &D2dCanvas<'_>, color: u32) -> Option<ID2D1SolidColorBrush> {
    let col = color_from_argb(color);
    g.rt.CreateSolidColorBrush(&col, None).ok()
}

unsafe fn make_linear_brush(
    g: &D2dCanvas<'_>,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    c1: u32,
    c2: u32,
) -> Option<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: color_from_argb(c1),
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: color_from_argb(c2),
        },
    ];
    let collection: ID2D1GradientStopCollection = g
        .rt
        .CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
        .ok()?;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
        startPoint: d2d_pt(x1, y1),
        endPoint: d2d_pt(x2, y2),
    };
    g.rt.CreateLinearGradientBrush(&props, None, &collection)
        .ok()
}

unsafe fn fill_round_rect(
    g: &D2dCanvas<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: u32,
) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let rx = x.round();
    let ry = y.round();
    let rw = w.round().max(1.0);
    let rh = h.round().max(1.0);
    let radius = r.clamp(0.0, (rw.min(rh) * 0.5).max(0.0));
    let rr = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: rx,
            top: ry,
            right: rx + rw,
            bottom: ry + rh,
        },
        radiusX: radius,
        radiusY: radius,
    };
    g.rt.FillRoundedRectangle(&rr, &brush);
}

unsafe fn stroke_round_rect(
    g: &D2dCanvas<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: u32,
    width: f32,
) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let half = width * 0.5;
    let rx = x.round() + half;
    let ry = y.round() + half;
    let rw = (w.round() - width).max(1.0);
    let rh = (h.round() - width).max(1.0);
    let radius = (r - half).clamp(0.0, (rw.min(rh) * 0.5).max(0.0));
    let rr = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: rx,
            top: ry,
            right: rx + rw,
            bottom: ry + rh,
        },
        radiusX: radius,
        radiusY: radius,
    };
    g.rt.DrawRoundedRectangle(&rr, &brush, width, None);
}

unsafe fn draw_text(
    g: &D2dCanvas<'_>,
    text: &str,
    font: &D2dFont,
    rect: RectF,
    color: u32,
    align: StringAlignment,
) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let layout = D2D_RECT_F {
        left: rect.X.round(),
        top: rect.Y.round(),
        right: (rect.X + rect.Width).round(),
        bottom: (rect.Y + rect.Height).round(),
    };
    g.rt.DrawText(
        &utf16,
        font.for_align(align),
        &layout,
        &brush,
        D2D1_DRAW_TEXT_OPTIONS_CLIP,
        DWRITE_MEASURING_MODE_NATURAL,
    );
}

struct DrawCtx {
    scale: f32,
    ox: f32,
    oy: f32,
    clip_y: Option<(f32, f32)>,
    hits: Vec<HitRegion>,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl DrawCtx {
    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF {
            X: ((x + self.ox) * self.scale).round(),
            Y: ((y + self.oy) * self.scale).round(),
            Width: (w * self.scale).round(),
            Height: (h * self.scale).round(),
        }
    }

    fn px(&self, v: f32) -> f32 {
        v * self.scale
    }

    fn push_hit(&mut self, x: f32, y: f32, w: f32, h: f32, action: HitAction) -> (bool, bool) {
        let (hy0, hy1) = match self.clip_y {
            Some((min_y, max_y)) => (y.max(min_y), (y + h).min(max_y)),
            None => (y, y + h),
        };
        if hy1 <= hy0 {
            return (false, false);
        }
        let idx = self.hits.len();
        self.hits.push(HitRegion {
            rect: RectF {
                X: x,
                Y: hy0,
                Width: w,
                Height: hy1 - hy0,
            },
            action,
        });
        (self.hovered == Some(idx), self.pressed == Some(idx))
    }
}

unsafe fn fill_polygon(g: &D2dCanvas<'_>, pts: &[windows_numerics::Vector2], brush: &ID2D1SolidColorBrush) {
    if pts.len() < 3 {
        return;
    }
    let Ok(path) = g.d2d.CreatePathGeometry() else {
        return;
    };
    let Ok(sink) = path.Open() else {
        return;
    };
    sink.BeginFigure(pts[0], D2D1_FIGURE_BEGIN_FILLED);
    for &p in &pts[1..] {
        sink.AddLine(p);
    }
    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
    let _ = sink.Close();
    g.rt.FillGeometry(&path, brush, None);
}

unsafe fn draw_music_note_icon(g: &D2dCanvas<'_>, c: &DrawCtx, x: f32, y: f32, size: f32) {
    let x0 = ((x + c.ox) * c.scale).round();
    let y0 = ((y + c.oy) * c.scale).round();
    let sz = (size * c.scale).round();
    let rad = (11.5 * c.scale).round();
    let rr = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x0,
            top: y0,
            right: x0 + sz,
            bottom: y0 + sz,
        },
        radiusX: rad,
        radiusY: rad,
    };
    if let Some(grad) = make_linear_brush(
        g,
        x0,
        y0,
        x0 + sz,
        y0 + sz,
        argb(255, 255, 42, 86),
        argb(255, 192, 10, 48),
    ) {
        g.rt.FillRoundedRectangle(&rr, &grad);
    }
    stroke_round_rect(
        g,
        x0,
        y0,
        sz,
        sz,
        rad,
        argb(56, 255, 255, 255),
        (1.0 * c.scale).max(1.0),
    );

    let Some(white) = make_solid_brush(g, argb(252, 255, 255, 255)) else {
        return;
    };
    let s = sz / 46.0;
    let pt = |dx: f32, dy: f32| d2d_pt(x0 + dx * s, y0 + dy * s);

    // Note heads
    g.rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: pt(15.2, 31.2),
            radiusX: 4.1 * s,
            radiusY: 3.1 * s,
        },
        &white,
    );
    g.rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: pt(30.4, 28.4),
            radiusX: 4.1 * s,
            radiusY: 3.1 * s,
        },
        &white,
    );
    // Crisp vector stems + top slanted beam
    let sw = (2.3 * s).max(1.5);
    g.rt.DrawLine(pt(18.1, 30.8), pt(18.1, 14.0), &white, sw, Some(g.stroke_style));
    g.rt.DrawLine(pt(33.3, 28.0), pt(33.3, 11.2), &white, sw, Some(g.stroke_style));
    let beam = [
        pt(16.95, 13.0),
        pt(34.45, 10.1),
        pt(34.45, 14.2),
        pt(16.95, 17.1),
    ];
    fill_polygon(g, &beam, &white);
}

unsafe fn draw_transport_glyph(
    g: &D2dCanvas<'_>,
    c: &DrawCtx,
    cx: f32,
    cy: f32,
    kind: &str,
    color: u32,
) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let x = ((cx + c.ox) * c.scale).round();
    let y = ((cy + c.oy) * c.scale).round();
    let s = c.scale;
    match kind {
        "prev" => {
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x - 5.5 * s,
                    top: y - 5.0 * s,
                    right: x - 3.3 * s,
                    bottom: y + 5.0 * s,
                },
                radiusX: 0.9 * s,
                radiusY: 0.9 * s,
            };
            g.rt.FillRoundedRectangle(&bar, &brush);
            let pts = [
                d2d_pt(x + 5.2 * s, y - 5.0 * s),
                d2d_pt(x - 2.3 * s, y),
                d2d_pt(x + 5.2 * s, y + 5.0 * s),
            ];
            fill_polygon(g, &pts, &brush);
        }
        "next" => {
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x + 3.3 * s,
                    top: y - 5.0 * s,
                    right: x + 5.5 * s,
                    bottom: y + 5.0 * s,
                },
                radiusX: 0.9 * s,
                radiusY: 0.9 * s,
            };
            g.rt.FillRoundedRectangle(&bar, &brush);
            let pts = [
                d2d_pt(x - 5.2 * s, y - 5.0 * s),
                d2d_pt(x + 2.3 * s, y),
                d2d_pt(x - 5.2 * s, y + 5.0 * s),
            ];
            fill_polygon(g, &pts, &brush);
        }
        "pause" => {
            let b1 = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x - 4.4 * s,
                    top: y - 5.2 * s,
                    right: x - 1.4 * s,
                    bottom: y + 5.2 * s,
                },
                radiusX: 1.0 * s,
                radiusY: 1.0 * s,
            };
            let b2 = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x + 1.4 * s,
                    top: y - 5.2 * s,
                    right: x + 4.4 * s,
                    bottom: y + 5.2 * s,
                },
                radiusX: 1.0 * s,
                radiusY: 1.0 * s,
            };
            g.rt.FillRoundedRectangle(&b1, &brush);
            g.rt.FillRoundedRectangle(&b2, &brush);
        }
        _ => {
            let pts = [
                d2d_pt(x - 3.8 * s, y - 5.6 * s),
                d2d_pt(x + 5.8 * s, y),
                d2d_pt(x - 3.8 * s, y + 5.6 * s),
            ];
            fill_polygon(g, &pts, &brush);
        }
    }
}

unsafe fn draw_chevron_right(g: &D2dCanvas<'_>, c: &DrawCtx, cx: f32, cy: f32, color: u32) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let x = ((cx + c.ox) * c.scale).round();
    let y = ((cy + c.oy) * c.scale).round();
    let s = c.scale;
    if let Ok(path) = g.d2d.CreatePathGeometry() {
        if let Ok(sink) = path.Open() {
            sink.BeginFigure(d2d_pt(x - 2.2 * s, y - 4.2 * s), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(d2d_pt(x + 2.2 * s, y));
            sink.AddLine(d2d_pt(x - 2.2 * s, y + 4.2 * s));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            g.rt.DrawGeometry(&path, &brush, (1.6 * s).max(1.3), Some(g.stroke_style));
        }
    }
}

unsafe fn draw_gear_icon(g: &D2dCanvas<'_>, c: &DrawCtx, cx: f32, cy: f32, color: u32) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let x = ((cx + c.ox) * c.scale).round();
    let y = ((cy + c.oy) * c.scale).round();
    let s = c.scale;
    let sw = (1.5 * s).max(1.2);
    // Inner ring
    g.rt.DrawEllipse(
        &D2D1_ELLIPSE {
            point: d2d_pt(x, y),
            radiusX: 3.9 * s,
            radiusY: 3.9 * s,
        },
        &brush,
        sw,
        None,
    );
    // Center dot
    g.rt.FillEllipse(
        &D2D1_ELLIPSE {
            point: d2d_pt(x, y),
            radiusX: 1.45 * s,
            radiusY: 1.45 * s,
        },
        &brush,
    );
    // 8 radial gear teeth
    for i in 0..8 {
        let ang = (i as f32) * std::f32::consts::FRAC_PI_4;
        let (sin_a, cos_a) = ang.sin_cos();
        g.rt.DrawLine(
            d2d_pt(x + cos_a * 4.3 * s, y + sin_a * 4.3 * s),
            d2d_pt(x + cos_a * 6.0 * s, y + sin_a * 6.0 * s),
            &brush,
            (1.7 * s).max(1.4),
            Some(g.stroke_style),
        );
    }
}

unsafe fn draw_reload_icon(g: &D2dCanvas<'_>, c: &DrawCtx, cx: f32, cy: f32, color: u32) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let x = ((cx + c.ox) * c.scale).round();
    let y = ((cy + c.oy) * c.scale).round();
    let s = c.scale;
    let r = 4.6 * s;
    if let Ok(path) = g.d2d.CreatePathGeometry() {
        if let Ok(sink) = path.Open() {
            // Start near top-right (-35 deg) and sweep clockwise 290 deg to top (-70 deg)
            let start = d2d_pt(x + r * 0.82, y - r * 0.57);
            let end = d2d_pt(x + r * 0.45, y - r * 0.89);
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_LARGE,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            g.rt.DrawGeometry(&path, &brush, (1.5 * s).max(1.2), Some(g.stroke_style));
        }
    }
    // Crisp arrow tip at start/top-right
    let tip = [
        d2d_pt(x + 4.9 * s, y - 5.2 * s),
        d2d_pt(x + 4.9 * s, y - 1.5 * s),
        d2d_pt(x + 1.3 * s, y - 2.3 * s),
    ];
    fill_polygon(g, &tip, &brush);
}

unsafe fn draw_plus_minus(g: &D2dCanvas<'_>, c: &DrawCtx, cx: f32, cy: f32, plus: bool, color: u32) {
    let Some(brush) = make_solid_brush(g, color) else {
        return;
    };
    let x = ((cx + c.ox) * c.scale).round();
    let y = ((cy + c.oy) * c.scale).round();
    let arm = (4.0 * c.scale).round();
    let sw = (1.6 * c.scale).max(1.3);
    g.rt.DrawLine(d2d_pt(x - arm, y), d2d_pt(x + arm, y), &brush, sw, Some(g.stroke_style));
    if plus {
        g.rt.DrawLine(d2d_pt(x, y - arm), d2d_pt(x, y + arm), &brush, sw, Some(g.stroke_style));
    }
}

unsafe fn draw_toggle(
    g: &D2dCanvas<'_>,
    c: &DrawCtx,
    x: f32,
    y: f32,
    checked: bool,
    enabled: bool,
) {
    let w = 36.0;
    let h = 20.0;
    let alpha_scale = if enabled { 255 } else { 115 };
    let track_color = if checked {
        argb(alpha_scale, 255, 34, 74)
    } else {
        argb(if enabled { 54 } else { 26 }, 255, 255, 255)
    };
    let rx = ((x + c.ox) * c.scale).round();
    let ry = ((y + c.oy) * c.scale).round();
    let rw = (w * c.scale).round();
    let rh = (h * c.scale).round();
    fill_round_rect(g, rx, ry, rw, rh, rh * 0.5, track_color);
    if !checked && enabled {
        stroke_round_rect(
            g,
            rx,
            ry,
            rw,
            rh,
            rh * 0.5,
            argb(38, 255, 255, 255),
            (1.0 * c.scale).max(1.0),
        );
    }
    let knob_r = (7.0 * c.scale).round();
    let knob_cx = if checked {
        rx + rw - (10.0 * c.scale).round()
    } else {
        rx + (10.0 * c.scale).round()
    };
    let knob_cy = ry + rh * 0.5;
    if let Some(knob) = make_solid_brush(g, argb(alpha_scale, 255, 255, 255)) {
        g.rt.FillEllipse(
            &D2D1_ELLIPSE {
                point: d2d_pt(knob_cx, knob_cy),
                radiusX: knob_r,
                radiusY: knob_r,
            },
            &knob,
        );
    }
}

unsafe fn draw_toggle_row(
    g: &D2dCanvas<'_>,
    c: &mut DrawCtx,
    fonts: &FontSet<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    badge: Option<(&str, u32)>,
    checked: bool,
    enabled: bool,
    cmd: &'static str,
) {
    let (hovered, pressed) = if enabled {
        c.push_hit(x, y, w, h, HitAction::Command(cmd))
    } else {
        (false, false)
    };
    let bg = if pressed {
        argb(255, 48, 48, 60)
    } else if hovered {
        argb(255, 39, 39, 50)
    } else {
        argb(255, 28, 28, 37)
    };
    let border = if hovered {
        argb(36, 255, 255, 255)
    } else {
        argb(16, 255, 255, 255)
    };
    fill_round_rect(
        g,
        (x + c.ox) * c.scale,
        (y + c.oy) * c.scale,
        c.px(w),
        c.px(h),
        c.px(9.0),
        bg,
    );
    stroke_round_rect(
        g,
        (x + c.ox) * c.scale,
        (y + c.oy) * c.scale,
        c.px(w),
        c.px(h),
        c.px(9.0),
        border,
        1.0,
    );
    let text_color = if enabled {
        argb(255, 245, 245, 250)
    } else {
        argb(255, 118, 118, 130)
    };
    let label_w = if badge.is_some() { w - 122.0 } else { w - 58.0 };
    draw_text(
        g,
        label,
        fonts.label.0,
        c.r(x + 12.0, y, label_w, h),
        text_color,
        StringAlignmentNear,
    );
    if let Some((b_text, b_color)) = badge {
        draw_text(
            g,
            b_text,
            fonts.sub.0,
            c.r(x + w - 112.0, y, 60.0, h),
            b_color,
            StringAlignmentFar,
        );
    }
    draw_toggle(g, c, x + w - 46.0, y + (h - 20.0) / 2.0, checked, enabled);
}

unsafe fn draw_action_row(
    g: &D2dCanvas<'_>,
    c: &mut DrawCtx,
    fonts: &FontSet<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    title: &str,
    sub: Option<&str>,
    danger: bool,
    cmd: &'static str,
) {
    let (hovered, pressed) = c.push_hit(x, y, w, h, HitAction::Command(cmd));
    let bg = if pressed {
        if danger {
            argb(255, 70, 26, 36)
        } else {
            argb(255, 48, 48, 60)
        }
    } else if hovered {
        if danger {
            argb(255, 56, 22, 30)
        } else {
            argb(255, 39, 39, 50)
        }
    } else if danger {
        argb(255, 38, 22, 27)
    } else {
        argb(255, 28, 28, 37)
    };
    let border = if danger {
        argb(if hovered { 95 } else { 52 }, 255, 92, 110)
    } else {
        argb(if hovered { 36 } else { 16 }, 255, 255, 255)
    };
    fill_round_rect(
        g,
        (x + c.ox) * c.scale,
        (y + c.oy) * c.scale,
        c.px(w),
        c.px(h),
        c.px(9.0),
        bg,
    );
    stroke_round_rect(
        g,
        (x + c.ox) * c.scale,
        (y + c.oy) * c.scale,
        c.px(w),
        c.px(h),
        c.px(9.0),
        border,
        1.0,
    );
    let fg = if danger {
        argb(255, 255, 152, 162)
    } else {
        argb(255, 245, 245, 250)
    };
    if let Some(sub_text) = sub {
        draw_text(
            g,
            title,
            fonts.label.0,
            c.r(x + 12.0, y + 4.0, w - 34.0, 20.0),
            fg,
            StringAlignmentNear,
        );
        draw_text(
            g,
            sub_text,
            fonts.sub.0,
            c.r(x + 12.0, y + 23.0, w - 34.0, 18.0),
            argb(255, 178, 178, 192),
            StringAlignmentNear,
        );
    } else {
        draw_text(
            g,
            title,
            fonts.label.0,
            c.r(x + 12.0, y, w - 34.0, h),
            fg,
            StringAlignmentNear,
        );
    }
    let chev_color = if danger {
        argb(255, 255, 148, 158)
    } else {
        argb(255, 175, 175, 190)
    };
    draw_chevron_right(g, c, x + w - 16.0, y + h * 0.5, chev_color);
}

struct RenderFrame<'a> {
    active_tab: PopupTab,
    scroll_y: f32,
    max_scroll: f32,
    hovered: Option<usize>,
    pressed: Option<usize>,
    hits: Vec<HitRegion>,
    snap: settings::Settings,
    track: Option<crate::presence::TrackMetadata>,
    win_visible: bool,
    blocked: u64,
    discord_status: &'a str,
}

fn rounded_rect_sdf(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, rad: f32) -> f32 {
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let hx = ((x1 - x0) * 0.5 - rad).max(0.0);
    let hy = ((y1 - y0) * 0.5 - rad).max(0.0);
    let dx = (px - cx).abs() - hx;
    let dy = (py - cy).abs() - hy;
    dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - rad
}

unsafe fn render_frame_to_dc(
    mem_dc: windows::Win32::Graphics::Gdi::HDC,
    bits: *mut u32,
    px_w: i32,
    px_h: i32,
    scale: f32,
    frame: &mut RenderFrame<'_>,
) {
    let card_w = (px_w as f32 / scale - SHADOW_PAD * 2.0).max(260.0);
    let card_h = (px_h as f32 / scale - SHADOW_PAD * 2.0).max(280.0);
    let total_pixels = (px_w as usize) * (px_h as usize);

    let _ = with_d2d_resources(scale, |d2d, stroke_style, bank| {
        let fonts = FontSet::from_bank(bank);
        let rc_full = RECT {
            left: 0,
            top: 0,
            right: px_w,
            bottom: px_h,
        };

        // Pass 1: Soft transparent drop shadow on PREMULTIPLIED target
        let props_shadow = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: Default::default(),
        };
        let Ok(rt_shadow): windows::core::Result<ID2D1DCRenderTarget> =
            d2d.CreateDCRenderTarget(&props_shadow)
        else {
            return;
        };
        let _ = rt_shadow.BindDC(mem_dc, &rc_full);
        let rt_s: &ID2D1RenderTarget = &rt_shadow;
        rt_s.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        rt_s.BeginDraw();
        let transparent = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        rt_s.Clear(Some(&transparent));
        let shadow_canvas = D2dCanvas {
            d2d,
            rt: rt_s,
            stroke_style,
        };
        for (expand, alpha) in [(10.0, 14), (7.0, 24), (4.0, 36), (1.5, 52)] {
            fill_round_rect(
                &shadow_canvas,
                (SHADOW_PAD - expand) * scale,
                (SHADOW_PAD - expand + 2.5) * scale,
                (card_w + expand * 2.0) * scale,
                (card_h + expand * 2.0) * scale,
                (CARD_RADIUS + expand * 0.6) * scale,
                argb(alpha, 0, 0, 0),
            );
        }
        let _ = rt_s.EndDraw(None, None);

        let shadow_buf = std::slice::from_raw_parts(bits, total_pixels).to_vec();

        // Pass 2: Full-clarity ClearType UI on opaque IGNORE target
        let props_clear = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: Default::default(),
        };
        let Ok(rt_ui): windows::core::Result<ID2D1DCRenderTarget> =
            d2d.CreateDCRenderTarget(&props_clear)
        else {
            return;
        };
        let _ = rt_ui.BindDC(mem_dc, &rc_full);
        let rt: &ID2D1RenderTarget = &rt_ui;
        rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
        rt.BeginDraw();
        rt.Clear(Some(&color_from_argb(argb(255, 24, 24, 31))));

        let canvas = D2dCanvas {
            d2d,
            rt,
            stroke_style,
        };
        let g_guard = CanvasHolder(&canvas);
        let mut c = DrawCtx {
            scale,
            ox: SHADOW_PAD,
            oy: SHADOW_PAD,
            clip_y: None,
            hits: Vec::with_capacity(40),
            hovered: frame.hovered,
            pressed: frame.pressed,
        };

        let card_px_x = (SHADOW_PAD * scale).round();
        let card_px_y = (SHADOW_PAD * scale).round();
        let card_px_w = (card_w * scale).round();
        let card_px_h = (card_h * scale).round();
        let card_rad_px = (CARD_RADIUS * scale).round();

        // Full-bleed gradient behind card so corner fringe blends cleanly without dark halo
        if let Some(bg_grad) = make_linear_brush(
            g_guard.0,
            card_px_x,
            card_px_y,
            card_px_x,
            card_px_y + card_px_h,
            argb(255, 29, 29, 38),
            argb(255, 19, 19, 26),
        ) {
            let full = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: px_w as f32,
                bottom: px_h as f32,
            };
            rt.FillRectangle(&full, &bg_grad);
        }

        let snap = &frame.snap;
        let track = &frame.track;
        let win_visible = frame.win_visible;
        let blocked = frame.blocked;
        let discord_status = frame.discord_status;

        // === Section 1: Now Playing Card ===
        let pad = 14.0;
        let inner_w = card_w - pad * 2.0;
        fill_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (14.0 + c.oy) * scale,
            c.px(inner_w),
            c.px(76.0),
            c.px(12.0),
            argb(255, 34, 34, 45),
        );
        stroke_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (14.0 + c.oy) * scale,
            c.px(inner_w),
            c.px(76.0),
            c.px(12.0),
            argb(28, 255, 255, 255),
            1.0,
        );
        draw_music_note_icon(g_guard.0, &c, pad + 11.0, 29.0, 46.0);

        let (state_text, state_dot) = match track.as_ref() {
            Some(t) if t.playing => ("ИГРАЕТ", argb(255, 68, 224, 138)),
            Some(_) => ("ПАУЗА", argb(255, 250, 178, 24)),
            None => ("ОЖИДАНИЕ", argb(255, 148, 148, 164)),
        };
        if let Some(dot_brush) = make_solid_brush(g_guard.0, state_dot) {
            rt.FillEllipse(
                &D2D1_ELLIPSE {
                    point: d2d_pt(
                        ((pad + 71.0 + c.ox) * scale).round(),
                        ((28.0 + c.oy) * scale).round(),
                    ),
                    radiusX: (3.2 * scale).round().max(2.5),
                    radiusY: (3.2 * scale).round().max(2.5),
                },
                &dot_brush,
            );
        }
        draw_text(
            g_guard.0,
            state_text,
            fonts.small_bold.0,
            c.r(pad + 78.0, 20.0, 110.0, 16.0),
            state_dot,
            StringAlignmentNear,
        );

        let title_str = track
            .as_ref()
            .map(|t| t.title.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Ничего не играет");
        let artist_str = track
            .as_ref()
            .and_then(|t| t.artist.as_deref())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(if track.is_some() {
                "YouTube Music"
            } else {
                "Включите трек в окне плеера"
            });

        // Show/Hide window pill
        let (vis_hov, vis_prs) = c.push_hit(
            pad + inner_w - 86.0,
            21.0,
            76.0,
            24.0,
            HitAction::Command(TRAY_SHOW_ID),
        );
        fill_round_rect(
            g_guard.0,
            (pad + inner_w - 86.0 + c.ox) * scale,
            (21.0 + c.oy) * scale,
            c.px(76.0),
            c.px(24.0),
            c.px(7.0),
            if vis_prs {
                argb(255, 68, 68, 84)
            } else if vis_hov {
                argb(255, 56, 56, 70)
            } else {
                argb(255, 45, 45, 57)
            },
        );
        stroke_round_rect(
            g_guard.0,
            (pad + inner_w - 86.0 + c.ox) * scale,
            (21.0 + c.oy) * scale,
            c.px(76.0),
            c.px(24.0),
            c.px(7.0),
            argb(34, 255, 255, 255),
            1.0,
        );
        draw_text(
            g_guard.0,
            if win_visible { "Открыть" } else { "Показать" },
            fonts.small_bold.0,
            c.r(pad + inner_w - 86.0, 21.0, 76.0, 24.0),
            argb(255, 250, 250, 255),
            StringAlignmentCenter,
        );

        draw_text(
            g_guard.0,
            title_str,
            fonts.title.0,
            c.r(pad + 68.0, 38.0, inner_w - 80.0, 22.0),
            argb(255, 255, 255, 255),
            StringAlignmentNear,
        );
        draw_text(
            g_guard.0,
            artist_str,
            fonts.sub.0,
            c.r(pad + 68.0, 60.0, inner_w - 80.0, 18.0),
            argb(255, 182, 182, 196),
            StringAlignmentNear,
        );

        // === Section 2: Transport Buttons ===
        let ty = 100.0;
        let th = 38.0;
        let gap = 8.0;
        let w_side = ((inner_w - gap * 2.0) * 0.30).round();
        let w_mid = inner_w - gap * 2.0 - w_side * 2.0;

        let (p_hov, p_prs) = c.push_hit(pad, ty, w_side, th, HitAction::Command(PREVIOUS_ID));
        fill_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (ty + c.oy) * scale,
            c.px(w_side),
            c.px(th),
            c.px(10.0),
            if p_prs {
                argb(255, 60, 60, 74)
            } else if p_hov {
                argb(255, 48, 48, 61)
            } else {
                argb(255, 36, 36, 46)
            },
        );
        stroke_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (ty + c.oy) * scale,
            c.px(w_side),
            c.px(th),
            c.px(10.0),
            argb(24, 255, 255, 255),
            1.0,
        );
        draw_transport_glyph(
            g_guard.0,
            &c,
            pad + 20.0,
            ty + th / 2.0,
            "prev",
            argb(255, 245, 245, 250),
        );
        draw_text(
            g_guard.0,
            "Назад",
            fonts.label.0,
            c.r(pad + 30.0, ty, w_side - 36.0, th),
            argb(255, 245, 245, 250),
            StringAlignmentCenter,
        );

        let mid_x = pad + w_side + gap;
        let (m_hov, m_prs) = c.push_hit(mid_x, ty, w_mid, th, HitAction::Command(PLAY_PAUSE_ID));
        fill_round_rect(
            g_guard.0,
            (mid_x + c.ox) * scale,
            (ty + c.oy) * scale,
            c.px(w_mid),
            c.px(th),
            c.px(10.0),
            if m_prs {
                argb(255, 200, 20, 56)
            } else if m_hov {
                argb(255, 255, 58, 92)
            } else {
                argb(255, 242, 30, 70)
            },
        );
        let is_playing = track.as_ref().is_some_and(|t| t.playing);
        draw_transport_glyph(
            g_guard.0,
            &c,
            mid_x + 28.0,
            ty + th / 2.0,
            if is_playing { "pause" } else { "play" },
            argb(255, 255, 255, 255),
        );
        draw_text(
            g_guard.0,
            if is_playing { "Пауза" } else { "Играть" },
            fonts.title.0,
            c.r(mid_x + 34.0, ty, w_mid - 42.0, th),
            argb(255, 255, 255, 255),
            StringAlignmentCenter,
        );

        let next_x = mid_x + w_mid + gap;
        let (n_hov, n_prs) = c.push_hit(next_x, ty, w_side, th, HitAction::Command(NEXT_ID));
        fill_round_rect(
            g_guard.0,
            (next_x + c.ox) * scale,
            (ty + c.oy) * scale,
            c.px(w_side),
            c.px(th),
            c.px(10.0),
            if n_prs {
                argb(255, 60, 60, 74)
            } else if n_hov {
                argb(255, 48, 48, 61)
            } else {
                argb(255, 36, 36, 46)
            },
        );
        stroke_round_rect(
            g_guard.0,
            (next_x + c.ox) * scale,
            (ty + c.oy) * scale,
            c.px(w_side),
            c.px(th),
            c.px(10.0),
            argb(24, 255, 255, 255),
            1.0,
        );
        draw_text(
            g_guard.0,
            "Далее",
            fonts.label.0,
            c.r(next_x + 6.0, ty, w_side - 36.0, th),
            argb(255, 245, 245, 250),
            StringAlignmentCenter,
        );
        draw_transport_glyph(
            g_guard.0,
            &c,
            next_x + w_side - 20.0,
            ty + th / 2.0,
            "next",
            argb(255, 245, 245, 250),
        );

        // === Section 3: Segmented Tabs ===
        let tab_y = 148.0;
        let tab_h = 32.0;
        fill_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (tab_y + c.oy) * scale,
            c.px(inner_w),
            c.px(tab_h),
            c.px(9.0),
            argb(255, 32, 32, 42),
        );
        let tab_w = (inner_w - 6.0) / 3.0;
        for (i, (tab, title)) in [
            (PopupTab::Main, "Главное"),
            (PopupTab::Features, "Функции"),
            (PopupTab::System, "Система"),
        ]
        .into_iter()
        .enumerate()
        {
            let tx = pad + 3.0 + tab_w * i as f32;
            let active = frame.active_tab == tab;
            let (t_hov, _) =
                c.push_hit(tx, tab_y + 3.0, tab_w, tab_h - 6.0, HitAction::SelectTab(tab));
            if active {
                fill_round_rect(
                    g_guard.0,
                    (tx + c.ox) * scale,
                    (tab_y + 3.0 + c.oy) * scale,
                    c.px(tab_w),
                    c.px(tab_h - 6.0),
                    c.px(7.0),
                    argb(255, 52, 52, 66),
                );
                stroke_round_rect(
                    g_guard.0,
                    (tx + c.ox) * scale,
                    (tab_y + 3.0 + c.oy) * scale,
                    c.px(tab_w),
                    c.px(tab_h - 6.0),
                    c.px(7.0),
                    argb(30, 255, 255, 255),
                    1.0,
                );
                fill_round_rect(
                    g_guard.0,
                    (tx + tab_w * 0.28 + c.ox) * scale,
                    (tab_y + tab_h - 5.5 + c.oy) * scale,
                    c.px(tab_w * 0.44),
                    c.px(2.0),
                    c.px(1.0),
                    argb(255, 255, 48, 88),
                );
            } else if t_hov {
                fill_round_rect(
                    g_guard.0,
                    (tx + c.ox) * scale,
                    (tab_y + 3.0 + c.oy) * scale,
                    c.px(tab_w),
                    c.px(tab_h - 6.0),
                    c.px(7.0),
                    argb(255, 40, 40, 52),
                );
            }
            draw_text(
                g_guard.0,
                title,
                if active { fonts.tab_active.0 } else { fonts.label.0 },
                c.r(tx, tab_y + 3.0, tab_w, tab_h - 6.0),
                if active {
                    argb(255, 255, 255, 255)
                } else {
                    argb(255, 186, 186, 200)
                },
                StringAlignmentCenter,
            );
        }

        // === Section 4: Scrollable / Tab Content ===
        let content_top = 190.0;
        let footer_top = card_h - 50.0;
        let content_h = (footer_top - content_top - 6.0).max(80.0);
        let clip_rect = D2D_RECT_F {
            left: ((pad + c.ox) * scale).round(),
            top: ((content_top + c.oy) * scale).round(),
            right: ((pad + inner_w + c.ox) * scale).round(),
            bottom: ((content_top + content_h + c.oy) * scale).round(),
        };
        rt.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        c.clip_y = Some((content_top, content_top + content_h));

        let mut cy = content_top;
        struct DataCompat {
            max_scroll: f32,
            scroll_y: f32,
        }
        let mut data = DataCompat {
            max_scroll: frame.max_scroll,
            scroll_y: frame.scroll_y,
        };
        match frame.active_tab {
            PopupTab::Main => {
                data.max_scroll = 0.0;
                data.scroll_y = 0.0;
                let row_h = 36.0;
                let row_gap = 6.0;
                let rpc_badge = if !snap.discord_rpc {
                    ("Выкл", argb(255, 152, 152, 166))
                } else if discord_status.starts_with("Подключено") {
                    ("Онлайн", argb(255, 68, 224, 138))
                } else {
                    ("Поиск", argb(255, 250, 178, 24))
                };
                draw_toggle_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    row_h,
                    "Discord Rich Presence",
                    Some(rpc_badge),
                    snap.discord_rpc,
                    true,
                    DISCORD_RPC_ID,
                );
                cy += row_h + row_gap;

                let ad_text = if snap.ad_block {
                    format!("Забл.: {blocked}")
                } else {
                    "Выкл".to_string()
                };
                let ad_color = if snap.ad_block {
                    argb(255, 182, 182, 196)
                } else {
                    argb(255, 152, 152, 166)
                };
                draw_toggle_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    row_h,
                    "Блокировка рекламы (AdBlock)",
                    Some((&ad_text, ad_color)),
                    snap.ad_block,
                    true,
                    AD_BLOCK_ID,
                );
                cy += row_h + row_gap;

                draw_toggle_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    row_h,
                    "Сворачивать в трей при закрытии",
                    None,
                    snap.close_to_tray,
                    true,
                    CLOSE_TO_TRAY_ID,
                );
                cy += row_h + row_gap;

                draw_toggle_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    row_h,
                    "Запускать вместе с Windows",
                    None,
                    snap.launch_at_startup,
                    true,
                    STARTUP_ID,
                );
                cy += row_h + row_gap;

                draw_toggle_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    row_h,
                    "Запускать свёрнутым в трей",
                    None,
                    snap.start_minimized,
                    snap.launch_at_startup,
                    START_MINIMIZED_ID,
                );
                cy += row_h + row_gap + 2.0;

                // Zoom + Reload bar
                fill_round_rect(
                    g_guard.0,
                    (pad + c.ox) * scale,
                    (cy + c.oy) * scale,
                    c.px(inner_w),
                    c.px(40.0),
                    c.px(9.0),
                    argb(255, 28, 28, 37),
                );
                stroke_round_rect(
                    g_guard.0,
                    (pad + c.ox) * scale,
                    (cy + c.oy) * scale,
                    c.px(inner_w),
                    c.px(40.0),
                    c.px(9.0),
                    argb(16, 255, 255, 255),
                    1.0,
                );
                draw_text(
                    g_guard.0,
                    "Масштаб",
                    fonts.label.0,
                    c.r(pad + 12.0, cy, 70.0, 40.0),
                    argb(255, 240, 240, 248),
                    StringAlignmentNear,
                );
                let z_pct = format!("{}%", (snap.zoom * 100.0).round() as i32);
                let mut bx = pad + 82.0;
                let reload_w = inner_w - 82.0 - 122.0 - 8.0;
                for (bw, label, cmd) in [
                    (28.0, "", ZOOM_OUT_ID),
                    (52.0, z_pct.as_str(), ZOOM_RESET_ID),
                    (28.0, "", ZOOM_IN_ID),
                    (reload_w, "Обновить", RELOAD_ID),
                ] {
                    let (bh, bp) = c.push_hit(bx, cy + 6.0, bw, 28.0, HitAction::Command(cmd));
                    fill_round_rect(
                        g_guard.0,
                        (bx + c.ox) * scale,
                        (cy + 6.0 + c.oy) * scale,
                        c.px(bw),
                        c.px(28.0),
                        c.px(7.0),
                        if bp {
                            argb(255, 62, 62, 76)
                        } else if bh {
                            argb(255, 50, 50, 62)
                        } else {
                            argb(255, 38, 38, 49)
                        },
                    );
                    stroke_round_rect(
                        g_guard.0,
                        (bx + c.ox) * scale,
                        (cy + 6.0 + c.oy) * scale,
                        c.px(bw),
                        c.px(28.0),
                        c.px(7.0),
                        argb(24, 255, 255, 255),
                        1.0,
                    );
                    if cmd == ZOOM_OUT_ID {
                        draw_plus_minus(
                            g_guard.0,
                            &c,
                            bx + bw * 0.5,
                            cy + 20.0,
                            false,
                            argb(255, 245, 245, 250),
                        );
                    } else if cmd == ZOOM_IN_ID {
                        draw_plus_minus(
                            g_guard.0,
                            &c,
                            bx + bw * 0.5,
                            cy + 20.0,
                            true,
                            argb(255, 245, 245, 250),
                        );
                    } else if cmd == RELOAD_ID {
                        draw_reload_icon(
                            g_guard.0,
                            &c,
                            bx + 17.0,
                            cy + 20.0,
                            argb(255, 235, 235, 245),
                        );
                        draw_text(
                            g_guard.0,
                            label,
                            fonts.sub.0,
                            c.r(bx + 22.0, cy + 6.0, bw - 26.0, 28.0),
                            argb(255, 245, 245, 250),
                            StringAlignmentCenter,
                        );
                    } else {
                        draw_text(
                            g_guard.0,
                            label,
                            fonts.sub.0,
                            c.r(bx, cy + 6.0, bw, 28.0),
                            argb(255, 245, 245, 250),
                            StringAlignmentCenter,
                        );
                    }
                    bx += bw + 5.0;
                }
            }
            PopupTab::Features => {
                let list_w = inner_w - 8.0;
                let scroll = clamp_scroll(data.scroll_y, data.max_scroll);
                data.scroll_y = scroll;
                cy = content_top - scroll;

                let add_section = |title: &str, c_ref: &DrawCtx, cy_ref: &mut f32| {
                    if *cy_ref + 22.0 >= content_top && *cy_ref <= content_top + content_h {
                        draw_text(
                            g_guard.0,
                            title,
                            fonts.section.0,
                            c_ref.r(pad + 4.0, *cy_ref, list_w, 20.0),
                            argb(210, 150, 150, 165),
                            StringAlignmentNear,
                        );
                    }
                    *cy_ref += 22.0;
                };

                let row_h = 33.0;
                let row_gap = 5.0;

                add_section("ТЕКСТ ПЕСНИ", &c, &mut cy);
                for (label, checked, cmd) in [
                    ("Синхронный текст песни", snap.synced_lyrics, SYNCED_LYRICS_ID),
                    ("Автоперевод текста", snap.lyrics_translation, LYRICS_TRANSLATION_ID),
                    ("Романизация (Romaji)", snap.lyrics_romanization, LYRICS_ROMANIZATION_ID),
                    ("Таймкоды строк", snap.lyrics_show_timecodes, LYRICS_TIMECODES_ID),
                    ("Точный тайминг (100 мс)", snap.lyrics_precise_timing, LYRICS_PRECISE_TIMING_ID),
                    ("Автосинхронизация (3 сек)", snap.lyrics_auto_sync, LYRICS_AUTO_SYNC_ID),
                ] {
                    if cy + row_h >= content_top && cy <= content_top + content_h {
                        draw_toggle_row(
                            g_guard.0,
                            &mut c,
                            &fonts,
                            pad,
                            cy,
                            list_w,
                            row_h,
                            label,
                            None,
                            checked,
                            true,
                            cmd,
                        );
                    }
                    cy += row_h + row_gap;
                }

                // Lyrics effect cycle row
                if cy + row_h >= content_top && cy <= content_top + content_h {
                    let eff_title = format!("Эффект строк: {}", lyrics_effect_label(&snap.lyrics_line_effect));
                    draw_action_row(
                        g_guard.0,
                        &mut c,
                        &fonts,
                        pad,
                        cy,
                        list_w,
                        row_h,
                        &eff_title,
                        None,
                        false,
                        CYCLE_LYRICS_EFFECT_ID,
                    );
                }
                cy += row_h + row_gap + 4.0;

                add_section("ЗВУК И ВОСПРОИЗВЕДЕНИЕ", &c, &mut cy);
                for (label, checked, cmd) in [
                    ("10-полосный эквалайзер", snap.equalizer, EQUALIZER_ID),
                    ("Плавный кроссфейд", snap.crossfade, CROSSFADE_ID),
                    ("SponsorBlock (пропуск вставок)", snap.sponsorblock, SPONSORBLOCK_ID),
                    ("Режим «Только аудио»", snap.video_toggle, VIDEO_TOGGLE_ID),
                    ("Точная громкость (шаг 1%)", snap.precise_volume, PRECISE_VOLUME_ID),
                    ("Экспоненциальная громкость", snap.exponential_volume, EXPONENTIAL_VOLUME_ID),
                    ("Слайдер скорости трека", snap.playback_speed, PLAYBACK_SPEED_ID),
                    ("Пропускать дизлайки", snap.skip_disliked, SKIP_DISLIKED_ID),
                    ("Отключить автовоспроизведение", snap.disable_autoplay, DISABLE_AUTOPLAY_ID),
                    ("Своё устройство вывода", snap.custom_output_device, CUSTOM_OUTPUT_DEVICE_ID),
                ] {
                    if cy + row_h >= content_top && cy <= content_top + content_h {
                        draw_toggle_row(
                            g_guard.0,
                            &mut c,
                            &fonts,
                            pad,
                            cy,
                            list_w,
                            row_h,
                            label,
                            None,
                            checked,
                            true,
                            cmd,
                        );
                    }
                    cy += row_h + row_gap;
                }

                cy += 4.0;
                add_section("ВНЕШНИЙ ВИД И ИНТЕГРАЦИИ", &c, &mut cy);
                for (label, checked, cmd) in [
                    ("Ambient-подсветка плеера", snap.ambient_mode, AMBIENT_MODE_ID),
                    ("Размытие панелей", snap.blur_nav_bar, BLUR_NAV_BAR_ID),
                    ("Кнопки Назад / Вперёд в окне", snap.navigation_controls, NAVIGATION_CONTROLS_ID),
                    ("Уведомления о смене трека", snap.notifications, NOTIFICATIONS_ID),
                    ("Кнопки в панели задач Windows", snap.windows_media_controls, WINDOWS_MEDIA_CONTROLS_ID),
                    ("Last.fm скроблинг", snap.lastfm_scrobbling, LASTFM_SCROBBLING_ID),
                    ("ListenBrainz скроблинг", snap.listenbrainz_scrobbling, LISTENBRAINZ_SCROBBLING_ID),
                    ("Показывать как Spotify в Discord", snap.spotify_spoof, SPOTIFY_SPOOF_ID),
                ] {
                    if cy + row_h >= content_top && cy <= content_top + content_h {
                        draw_toggle_row(
                            g_guard.0,
                            &mut c,
                            &fonts,
                            pad,
                            cy,
                            list_w,
                            row_h,
                            label,
                            None,
                            checked,
                            true,
                            cmd,
                        );
                    }
                    cy += row_h + row_gap;
                }

                let total_h = (cy + scroll) - content_top;
                data.max_scroll = (total_h - content_h).max(0.0);
                data.scroll_y = clamp_scroll(scroll, data.max_scroll);

                if data.max_scroll > 1.0 {
                    let sb_h = (content_h * (content_h / total_h)).clamp(28.0, content_h - 8.0);
                    let sb_y = content_top + (content_h - sb_h) * (data.scroll_y / data.max_scroll);
                    fill_round_rect(
                        g_guard.0,
                        (pad + inner_w - 4.0 + c.ox) * scale,
                        (sb_y + c.oy) * scale,
                        c.px(4.0),
                        c.px(sb_h),
                        c.px(2.0),
                        argb(110, 255, 255, 255),
                    );
                }
            }
            PopupTab::System => {
                data.max_scroll = 0.0;
                data.scroll_y = 0.0;

                draw_action_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    44.0,
                    "Статус Discord RPC",
                    Some(discord_status),
                    false,
                    DISCORD_STATUS_ID,
                );
                cy += 50.0;

                let ad_sub = format!(
                    "{} · Заблокировано запросов: {}",
                    if snap.ad_block { "Включена" } else { "Выключена" },
                    blocked
                );
                draw_action_row(
                    g_guard.0,
                    &mut c,
                    &fonts,
                    pad,
                    cy,
                    inner_w,
                    44.0,
                    "Статистика блокировки рекламы",
                    Some(&ad_sub),
                    false,
                    AD_BLOCK_STATUS_ID,
                );
                cy += 50.0;

                for (title, danger, cmd) in [
                    ("Очистить кэш и перезагрузить", false, CLEAR_CACHE_ID),
                    ("Проверить обновления", false, CHECK_UPDATES_ID),
                    ("Инструменты разработчика (F12)", false, DEVTOOLS_ID),
                    ("Сбросить сессию (выйти из аккаунта)", true, RESET_SESSION_ID),
                ] {
                    draw_action_row(
                        g_guard.0,
                        &mut c,
                        &fonts,
                        pad,
                        cy,
                        inner_w,
                        35.0,
                        title,
                        None,
                        danger,
                        cmd,
                    );
                    cy += 40.0;
                }
            }
        }
        c.clip_y = None;
        rt.PopAxisAlignedClip();
        frame.max_scroll = data.max_scroll;
        frame.scroll_y = data.scroll_y;

        // === Section 5: Footer ===
        if let Some(div_brush) = make_solid_brush(g_guard.0, argb(28, 255, 255, 255)) {
            let fy_line = ((footer_top - 4.0 + c.oy) * scale).round() + 0.5;
            rt.DrawLine(
                d2d_pt(((pad + c.ox) * scale).round(), fy_line),
                d2d_pt(((pad + inner_w + c.ox) * scale).round(), fy_line),
                &div_brush,
                1.0,
                None,
            );
        }

        let fy = footer_top + 4.0;
        let fh = 34.0;
        let set_w = inner_w * 0.61;
        let quit_w = inner_w - set_w - 8.0;

        let (s_hov, s_prs) = c.push_hit(pad, fy, set_w, fh, HitAction::Command(OPEN_SETTINGS_ID));
        fill_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (fy + c.oy) * scale,
            c.px(set_w),
            c.px(fh),
            c.px(9.0),
            if s_prs {
                argb(255, 62, 62, 76)
            } else if s_hov {
                argb(255, 48, 48, 61)
            } else {
                argb(255, 36, 36, 46)
            },
        );
        stroke_round_rect(
            g_guard.0,
            (pad + c.ox) * scale,
            (fy + c.oy) * scale,
            c.px(set_w),
            c.px(fh),
            c.px(9.0),
            argb(26, 255, 255, 255),
            1.0,
        );
        draw_gear_icon(
            g_guard.0,
            &c,
            pad + 28.0,
            fy + fh * 0.5,
            argb(255, 232, 232, 244),
        );
        draw_text(
            g_guard.0,
            "Все настройки",
            fonts.label.0,
            c.r(pad + 34.0, fy, set_w - 40.0, fh),
            argb(255, 245, 245, 250),
            StringAlignmentCenter,
        );

        let qx = pad + set_w + 8.0;
        let (q_hov, q_prs) = c.push_hit(qx, fy, quit_w, fh, HitAction::Command(TRAY_QUIT_ID));
        fill_round_rect(
            g_guard.0,
            (qx + c.ox) * scale,
            (fy + c.oy) * scale,
            c.px(quit_w),
            c.px(fh),
            c.px(9.0),
            if q_prs {
                argb(255, 86, 28, 38)
            } else if q_hov {
                argb(255, 68, 24, 34)
            } else {
                argb(255, 48, 22, 28)
            },
        );
        stroke_round_rect(
            g_guard.0,
            (qx + c.ox) * scale,
            (fy + c.oy) * scale,
            c.px(quit_w),
            c.px(fh),
            c.px(9.0),
            argb(if q_hov { 110 } else { 68 }, 255, 92, 110),
            1.0,
        );
        draw_text(
            g_guard.0,
            "Выход",
            fonts.label.0,
            c.r(qx, fy, quit_w, fh),
            argb(255, 255, 162, 172),
            StringAlignmentCenter,
        );

        // Crisp 1px card border drawn before corner masking
        stroke_round_rect(
            g_guard.0,
            card_px_x,
            card_px_y,
            card_px_w,
            card_px_h,
            card_rad_px,
            argb(46, 255, 255, 255),
            1.0,
        );

        let _ = rt.EndDraw(None, None);

        // Composite ClearType opaque UI + antialiased rounded corner over shadow alpha buffer
        let dst = std::slice::from_raw_parts_mut(bits, total_pixels);
        let x0 = card_px_x;
        let y0 = card_px_y;
        let x1 = card_px_x + card_px_w;
        let y1 = card_px_y + card_px_h;
        for py in 0..px_h {
            let pyf = py as f32 + 0.5;
            let row_off = (py as usize) * (px_w as usize);
            for px in 0..px_w {
                let idx = row_off + (px as usize);
                let dist = rounded_rect_sdf(px as f32 + 0.5, pyf, x0, y0, x1, y1, card_rad_px);
                if dist <= -0.5 {
                    dst[idx] |= 0xFF00_0000;
                } else if dist >= 0.5 {
                    dst[idx] = shadow_buf[idx];
                } else {
                    let cov = (0.5 - dist).clamp(0.0, 1.0);
                    let fg = dst[idx];
                    let sh = shadow_buf[idx];
                    let a_sh = ((sh >> 24) & 0xFF) as f32 / 255.0;
                    let a_out = (cov + (1.0 - cov) * a_sh).clamp(0.0, 1.0);
                    let a_u8 = (a_out * 255.0).round() as u32;
                    let r_fg = ((fg >> 16) & 0xFF) as f32;
                    let g_fg = ((fg >> 8) & 0xFF) as f32;
                    let b_fg = (fg & 0xFF) as f32;
                    let r_sh = ((sh >> 16) & 0xFF) as f32;
                    let g_sh = ((sh >> 8) & 0xFF) as f32;
                    let b_sh = (sh & 0xFF) as f32;
                    let r = (cov * r_fg + (1.0 - cov) * r_sh).round().min(a_u8 as f32) as u32;
                    let g_c = (cov * g_fg + (1.0 - cov) * g_sh).round().min(a_u8 as f32) as u32;
                    let b = (cov * b_fg + (1.0 - cov) * b_sh).round().min(a_u8 as f32) as u32;
                    dst[idx] = (a_u8 << 24) | (r << 16) | (g_c << 8) | b;
                }
            }
        }

        frame.hits = c.hits;
    });
}

unsafe fn render_hwnd(hwnd: HWND, data: &mut PopupData) {
    let scale = dpi_scale(hwnd);
    let mut win_rect = RECT::default();
    let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut win_rect);
    let px_w = (win_rect.right - win_rect.left)
        .max(((POPUP_W + SHADOW_PAD * 2.0) * scale).round() as i32);
    let px_h = (win_rect.bottom - win_rect.top).max(320);

    let screen_dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(Some(screen_dc));
    let mut bits: *mut c_void = ptr::null_mut();
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: px_w,
            biHeight: -px_h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let Ok(dib) = CreateDIBSection(Some(screen_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
    else {
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
        return;
    };
    let old_obj = SelectObject(mem_dc, dib.into());

    let snap = settings::snapshot(&data.state.settings);
    let track = controls::current_track(&data.app);
    let win_visible = controls::is_window_visible(&data.app);
    let blocked = data.state.adblock.blocked_requests();
    let discord_status = data.state.presence.status();

    let mut frame = RenderFrame {
        active_tab: data.active_tab,
        scroll_y: data.scroll_y,
        max_scroll: data.max_scroll,
        hovered: data.hovered,
        pressed: data.pressed,
        hits: Vec::with_capacity(40),
        snap,
        track,
        win_visible,
        blocked,
        discord_status: &discord_status,
    };
    render_frame_to_dc(mem_dc, bits.cast::<u32>(), px_w, px_h, scale, &mut frame);
    data.scroll_y = frame.scroll_y;
    data.max_scroll = frame.max_scroll;
    data.hits = frame.hits;

    let pt_dst = POINT {
        x: win_rect.left,
        y: win_rect.top,
    };
    let size = SIZE { cx: px_w, cy: px_h };
    let pt_src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        Some(screen_dc),
        Some(&pt_dst),
        Some(&size),
        Some(mem_dc),
        Some(&pt_src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(mem_dc, old_obj);
    let _ = DeleteObject(dib.into());
    let _ = DeleteDC(mem_dc);
    ReleaseDC(None, screen_dc);
}

#[cfg(test)]
mod render_tests {
    use super::*;

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn adler32(data: &[u8]) -> u32 {
        let mut a = 1u32;
        let mut b = 0u32;
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn push_png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(body);
        let mut crc_in = Vec::with_capacity(4 + body.len());
        crc_in.extend_from_slice(tag);
        crc_in.extend_from_slice(body);
        out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
    }

    fn write_bgra_png(path: &std::path::Path, w: i32, h: i32, bgra: &[u32]) {
        let wu = w as usize;
        let hu = h as usize;
        let mut raw = Vec::with_capacity(hu * (1 + wu * 4));
        for y in 0..hu {
            raw.push(0);
            for x in 0..wu {
                let px = bgra[y * wu + x];
                let a = ((px >> 24) & 0xFF) as u8;
                let mut r = ((px >> 16) & 0xFF) as u8;
                let mut g = ((px >> 8) & 0xFF) as u8;
                let mut b = (px & 0xFF) as u8;
                if a > 0 && a < 255 {
                    let af = a as u16;
                    r = ((r as u16 * 255) / af).min(255) as u8;
                    g = ((g as u16 * 255) / af).min(255) as u8;
                    b = ((b as u16 * 255) / af).min(255) as u8;
                }
                raw.extend_from_slice(&[r, g, b, a]);
            }
        }
        let mut zlib = Vec::with_capacity(raw.len() + 64);
        zlib.extend_from_slice(&[0x78, 0x01]);
        let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            zlib.push(if i + 1 == chunks.len() { 1 } else { 0 });
            let len = chunk.len() as u16;
            zlib.extend_from_slice(&len.to_le_bytes());
            zlib.extend_from_slice(&(!len).to_le_bytes());
            zlib.extend_from_slice(chunk);
        }
        zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut png = Vec::with_capacity(zlib.len() + 64);
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        push_png_chunk(&mut png, b"IHDR", &ihdr);
        push_png_chunk(&mut png, b"IDAT", &zlib);
        push_png_chunk(&mut png, b"IEND", &[]);
        let _ = std::fs::write(path, png);
    }

    fn render_offscreen(tab: PopupTab, scale: f32, playing: bool) -> (i32, i32, Vec<u32>, usize) {
        unsafe {
            let px_w = ((POPUP_W + SHADOW_PAD * 2.0) * scale).round() as i32;
            let px_h = ((POPUP_H + SHADOW_PAD * 2.0) * scale).round() as i32;
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let mut bits: *mut c_void = ptr::null_mut();
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: px_w,
                    biHeight: -px_h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let dib =
                CreateDIBSection(Some(screen_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
            let old_obj = SelectObject(mem_dc, dib.into());

            let track = if playing {
                Some(crate::presence::TrackMetadata {
                    title: "Tesla Ultra Slowed (Electro Mix)".into(),
                    artist: Some("SalihBademci".into()),
                    album: None,
                    url: None,
                    cover_url: None,
                    playing: true,
                    elapsed_seconds: None,
                    duration_seconds: None,
                })
            } else {
                None
            };
            let mut frame = RenderFrame {
                active_tab: tab,
                scroll_y: 0.0,
                max_scroll: 0.0,
                hovered: None,
                pressed: None,
                hits: Vec::new(),
                snap: settings::Settings::default(),
                track,
                win_visible: true,
                blocked: 12,
                discord_status: "Включено: ожидание Discord",
            };
            render_frame_to_dc(mem_dc, bits.cast::<u32>(), px_w, px_h, scale, &mut frame);
            let pixels = std::slice::from_raw_parts(bits.cast::<u32>(), (px_w * px_h) as usize).to_vec();

            SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
            (px_w, px_h, pixels, frame.hits.len())
        }
    }

    #[test]
    fn renders_crisp_direct2d_tabs_without_blank_output() {
        let dump_dir = std::env::var_os("YTM_DUMP_PREVIEW_DIR").map(std::path::PathBuf::from);
        for (tab, name, playing) in [
            (PopupTab::Main, "main", false),
            (PopupTab::Features, "features", true),
            (PopupTab::System, "system", true),
        ] {
            for scale in [1.0f32, 1.5f32] {
                let (w, h, px, hits) = render_offscreen(tab, scale, playing);
                assert!(hits >= 8, "tab {name} rendered too few hit regions: {hits}");
                let center = px[((h / 2) * w + (w / 2)) as usize];
                assert_eq!((center >> 24) & 0xFF, 255, "interior alpha must be opaque");
                assert_eq!((px[0] >> 24) & 0xFF, 0, "outer corner must be transparent");
                if let Some(dir) = &dump_dir {
                    let _ = std::fs::create_dir_all(dir);
                    write_bgra_png(
                        &dir.join(format!("preview_{name}_{}.png", (scale * 100.0) as i32)),
                        w,
                        h,
                        &px,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_origin_clamps_above_taskbar_and_inside_work_area() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1032,
        };
        let (x, y) = compute_popup_origin(POINT { x: 1890, y: 1055 }, (384, 532), work, 14);
        assert!(x + 14 >= 0 && x + 384 - 14 <= 1920);
        assert!(y + 14 >= 0 && y + 532 - 14 <= 1032);
    }

    #[test]
    fn scroll_clamps_to_valid_range() {
        assert_eq!(clamp_scroll(-20.0, 120.0), 0.0);
        assert_eq!(clamp_scroll(50.0, 120.0), 50.0);
        assert_eq!(clamp_scroll(400.0, 120.0), 120.0);
        assert_eq!(clamp_scroll(10.0, 0.0), 0.0);
    }

    #[test]
    fn hit_test_matches_topmost_region() {
        let hits = vec![
            HitRegion {
                rect: RectF {
                    X: 10.0,
                    Y: 10.0,
                    Width: 100.0,
                    Height: 50.0,
                },
                action: HitAction::SelectTab(PopupTab::Main),
            },
            HitRegion {
                rect: RectF {
                    X: 20.0,
                    Y: 20.0,
                    Width: 40.0,
                    Height: 20.0,
                },
                action: HitAction::Command(PREVIOUS_ID),
            },
        ];
        assert_eq!(hit_test(&hits, 30.0, 25.0), Some(1));
        assert_eq!(hit_test(&hits, 90.0, 45.0), Some(0));
        assert_eq!(hit_test(&hits, 2.0, 2.0), None);
    }

    #[test]
    fn scrolled_content_clip_does_not_overlap_tabs() {
        let mut c = DrawCtx {
            scale: 1.0,
            ox: 14.0,
            oy: 14.0,
            clip_y: None,
            hits: Vec::new(),
            hovered: None,
            pressed: None,
        };
        c.push_hit(17.0, 151.0, 110.0, 26.0, HitAction::SelectTab(PopupTab::Main));
        c.clip_y = Some((190.0, 468.0));
        // Scrolled-up row positioned behind tabs (140..176) is fully outside clip_y
        c.push_hit(14.0, 140.0, 344.0, 36.0, HitAction::Command(SYNCED_LYRICS_ID));
        // Partially visible top row (172..208) only accepts clicks >= 190
        c.push_hit(14.0, 172.0, 344.0, 36.0, HitAction::Command(LYRICS_TRANSLATION_ID));
        c.clip_y = None;

        assert_eq!(hit_test(&c.hits, 50.0, 162.0), Some(0));
        assert_eq!(c.hits.len(), 2);
        assert_eq!(hit_test(&c.hits, 50.0, 180.0), None);
        assert_eq!(hit_test(&c.hits, 50.0, 198.0), Some(1));
    }
}
