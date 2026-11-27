#[cfg(windows)]
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, COREWEBVIEW2_PROCESS_FAILED_KIND,
        COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    },
    ProcessFailedEventHandler,
};

#[cfg(windows)]
use windows::{
    core::{Interface, GUID, HSTRING, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, PROPERTYKEY, RECT},
        System::{
            Com::StructuredStorage::PROPVARIANT,
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::EmptyWorkingSet,
            Threading::{
                AttachThreadInput, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
                OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
            },
        },
        UI::{
            Shell::{
                PropertiesSystem::{
                    IPropertyStore, PSCoerceToCanonicalValue, SHGetPropertyStoreForWindow,
                },
                ShellExecuteW,
            },
            WindowsAndMessaging::{
                AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetWindowRect,
                GetWindowThreadProcessId, IsIconic, MessageBoxW, SetForegroundWindow, SetWindowPos,
                ShowWindow, ASFW_ANY, HWND_TOP, IDYES, MB_ICONERROR, MB_ICONINFORMATION,
                MB_ICONQUESTION, MB_OK, MB_YESNO, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
                SW_RESTORE, SW_SHOW, SW_SHOWNORMAL,
            },
        },
    },
};

#[cfg(windows)]
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const APP_NAME: &str = "YouTube Music";
const APP_USER_MODEL_ID: &str = "app.ytmusic.desktop";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
pub fn open_url(url: &str) -> bool {
    if !url.starts_with("https://")
        && !url.starts_with("http://127.0.0.1:")
        && !url.starts_with("http://localhost:")
    {
        return false;
    }

    let operation = HSTRING::from("open");
    let target = HSTRING::from(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };

    result.0 as isize > 32
}

#[cfg(not(windows))]
pub fn open_url(_url: &str) -> bool {
    false
}

#[cfg(windows)]
pub fn info(title: &str, message: &str) {
    message_box(title, message, MB_OK | MB_ICONINFORMATION);
}

#[cfg(not(windows))]
pub fn info(_title: &str, _message: &str) {}

#[cfg(windows)]
pub fn error(title: &str, message: &str) {
    message_box(title, message, MB_OK | MB_ICONERROR);
}

#[cfg(not(windows))]
pub fn error(_title: &str, _message: &str) {}

#[cfg(windows)]
pub fn confirm(title: &str, message: &str) -> bool {
    message_box(title, message, MB_YESNO | MB_ICONQUESTION) == IDYES.0
}

#[cfg(not(windows))]
pub fn confirm(_title: &str, _message: &str) -> bool {
    false
}

#[cfg(windows)]
fn message_box(
    title: &str,
    message: &str,
    style: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
) -> i32 {
    let title = HSTRING::from(title);
    let message = HSTRING::from(message);

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
        )
        .0
    }
}

#[cfg(windows)]
pub fn startup_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .ok()
        .and_then(|key| key.get_value::<String, _>(APP_NAME).ok())
        .is_some()
}

#[cfg(not(windows))]
pub fn startup_enabled() -> bool {
    false
}

#[cfg(windows)]
pub fn set_startup_enabled(enabled: bool, minimized: bool) -> std::io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;

    if enabled {
        let executable = std::env::current_exe()?;
        let minimized = if minimized { " --minimized" } else { "" };
        key.set_value(
            APP_NAME,
            &format!("\"{}\"{minimized}", executable.display()),
        )
    } else {
        match key.delete_value(APP_NAME) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(windows)]
pub fn register_app_identity() -> std::io::Result<()> {
    let classes = RegKey::predef(HKEY_CURRENT_USER);
    let key_path = format!(r"Software\Classes\AppUserModelId\{APP_USER_MODEL_ID}");
    let (key, _) = classes.create_subkey(key_path)?;
    let executable = std::env::current_exe()?;

    key.set_value("DisplayName", &APP_NAME)?;
    let registered_icon = key
        .get_value::<String, _>("IconUri")
        .ok()
        .filter(|path| std::path::Path::new(path).is_file());
    if registered_icon.is_none() {
        key.set_value("IconUri", &executable.to_string_lossy().as_ref())?;
    }
    key.set_value("ShowInSettings", &1_u32)
}

#[cfg(windows)]
pub fn set_window_app_identity(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let property_key = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };
    let mut value = PROPVARIANT::from(APP_USER_MODEL_ID);

    unsafe {
        PSCoerceToCanonicalValue(&property_key, &mut value).map_err(|error| error.to_string())?;
        let store: IPropertyStore =
            SHGetPropertyStoreForWindow(hwnd).map_err(|error| error.to_string())?;
        store
            .SetValue(&property_key, &value)
            .map_err(|error| error.to_string())?;
        store.Commit().map_err(|error| error.to_string())
    }
}

#[cfg(windows)]
pub fn set_memory_usage_low(webview: tauri::webview::PlatformWebview, low: bool) {
    unsafe {
        let Ok(core_webview) = webview.controller().CoreWebView2() else {
            return;
        };
        if let Ok(wv19) = core_webview.cast::<ICoreWebView2_19>() {
            let level = if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = wv19.SetMemoryUsageTargetLevel(level);
        }
    }
}

#[cfg(windows)]
pub fn install_process_recovery(webview: tauri::webview::PlatformWebview) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_RELOAD_SECS: AtomicU64 = AtomicU64::new(0);

    unsafe {
        let Ok(core_webview) = webview.controller().CoreWebView2() else {
            return;
        };
        let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            if args.ProcessFailedKind(&mut kind).is_ok()
                && (kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                    || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED)
            {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let prev = LAST_RELOAD_SECS.load(Ordering::Relaxed);
                if now.saturating_sub(prev) >= 15
                    && LAST_RELOAD_SECS
                        .compare_exchange(prev, now, Ordering::SeqCst, Ordering::Relaxed)
                        .is_ok()
                {
                    if let Some(wv) = sender {
                        let _ = wv.Reload();
                    }
                }
            }
            Ok(())
        }));
        let mut token = 0;
        let _ = core_webview.add_ProcessFailed(&handler, &mut token);
    }
}

#[cfg(windows)]
pub fn trim_working_sets() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TRIM_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
    static LAST_TRIM_MS: AtomicU64 = AtomicU64::new(0);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let prev_ms = LAST_TRIM_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(prev_ms) < 800 {
        return;
    }
    if TRIM_IN_FLIGHT
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    LAST_TRIM_MS.store(now_ms, Ordering::Relaxed);

    unsafe {
        let root_pid = GetCurrentProcessId();
        let _ = EmptyWorkingSet(GetCurrentProcess());

        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            if !snapshot.is_invalid() {
                let mut pairs: Vec<(u32, u32)> = Vec::with_capacity(128);
                let mut entry = PROCESSENTRY32W {
                    dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                    ..Default::default()
                };
                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        pairs.push((entry.th32ParentProcessID, entry.th32ProcessID));
                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snapshot);

                let mut tree_pids: Vec<u32> = Vec::with_capacity(16);
                tree_pids.push(root_pid);
                let mut idx = 0;
                while idx < tree_pids.len() {
                    let parent = tree_pids[idx];
                    for &(ppid, pid) in &pairs {
                        if ppid == parent && !tree_pids.contains(&pid) {
                            tree_pids.push(pid);
                        }
                    }
                    idx += 1;
                }

                for &pid in tree_pids.iter().skip(1) {
                    if let Ok(handle) =
                        OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, pid)
                    {
                        if !handle.is_invalid() {
                            let _ = EmptyWorkingSet(handle);
                            let _ = CloseHandle(handle);
                        }
                    }
                }
            }
        }
    }

    TRIM_IN_FLIGHT.store(false, Ordering::Release);
}

#[cfg(windows)]
pub fn allow_foreground_any() {
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
}

#[cfg(windows)]
pub fn force_show_window(window: &tauri::WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let cmd = if IsIconic(hwnd).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(hwnd, cmd);

        // Recover if minimized state ever saved off-screen coordinates (e.g. -32000).
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() && (rect.left < -10000 || rect.top < -10000) {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                120,
                80,
                0,
                0,
                SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        } else {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }

        let fg = GetForegroundWindow();
        if !fg.is_invalid() && fg != hwnd {
            let fg_tid = GetWindowThreadProcessId(fg, None);
            let cur_tid = GetCurrentThreadId();
            if fg_tid != 0 && fg_tid != cur_tid {
                let attached = AttachThreadInput(cur_tid, fg_tid, true).as_bool();
                let _ = SetForegroundWindow(hwnd);
                let _ = BringWindowToTop(hwnd);
                if attached {
                    let _ = AttachThreadInput(cur_tid, fg_tid, false);
                }
                return;
            }
        }

        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
    }
}

#[cfg(not(windows))]
pub fn allow_foreground_any() {}

#[cfg(not(windows))]
pub fn force_show_window(_window: &tauri::WebviewWindow) {}

#[cfg(windows)]
pub fn trim_working_sets_async() {
    std::thread::spawn(trim_working_sets);
}

#[cfg(not(windows))]
pub fn trim_working_sets() {}

#[cfg(not(windows))]
pub fn trim_working_sets_async() {}

#[cfg(not(windows))]
pub fn install_process_recovery(_webview: tauri::webview::PlatformWebview) {}

#[cfg(not(windows))]
pub fn set_memory_usage_low(_webview: tauri::webview::PlatformWebview, _low: bool) {}

#[cfg(not(windows))]
pub fn register_app_identity() -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn set_window_app_identity(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn set_startup_enabled(_enabled: bool, _minimized: bool) -> std::io::Result<()> {
    Ok(())
}
