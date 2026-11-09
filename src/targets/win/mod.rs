use super::{Display, Target};
use anyhow::{Context as _, Result};
use windows::Win32::UI::HiDpi::{MDT_EFFECTIVE_DPI, MONITOR_DPI_TYPE};
use windows::core::HRESULT;
use windows::Win32::{
    Foundation::{HWND, RECT, S_OK},
    Graphics::Gdi::{GetDeviceCaps, HMONITOR, LOGPIXELSX, GetDC, ReleaseDC},
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
};
use windows_capture::{
    monitor::{Error as MonitorError, Monitor},
    window::Window,
};

pub fn get_all_targets() -> Result<Vec<Target>> {
    let mut targets: Vec<Target> = Vec::new();

    // Add displays to targets
    let displays = Monitor::enumerate().context("Failed to enumerate monitors")?;
    for display in displays {
        let id = display.as_raw_hmonitor() as u32;
        let title = monitor_title(&display).context("Failed to get monitor name")?;

        let target = Target::Display(super::Display {
            id,
            title,
            raw_handle: HMONITOR(display.as_raw_hmonitor()),
            width: display.width()? as u16,
            height: display.height()? as u16,
        });
        targets.push(target);
    }

    // Add windows to targets
    let windows = Window::enumerate().context("Failed to enumerate windows")?;
    for window in windows {
        let id = window.as_raw_hwnd() as u32;
        let title = window
            .title()
            .context("Window title not found")?
            .to_string();

        let target = Target::Window(super::Window {
            id,
            title,
            raw_handle: HWND(window.as_raw_hwnd()),
        });
        targets.push(target);
    }

    Ok(targets)
}

pub fn get_main_display() -> Result<Display> {
    let display = Monitor::primary().context("Failed to get primary monitor")?;
    let id = display.as_raw_hmonitor() as u32;

    Ok(Display {
        id,
        title: monitor_title(&display).context("Failed to get monitor name")?,
        raw_handle: HMONITOR(display.as_raw_hmonitor()),
        width: display.width()? as u16,
        height: display.height()? as u16,
    })
}

fn monitor_title(monitor: &Monitor) -> Result<String, MonitorError> {
    monitor
        .name()
        .or_else(|_| monitor.device_string())
        .or_else(|_| monitor.device_name())
}

type GetDpiForWindowFn = unsafe extern "system" fn(HWND) -> u32;

/// `GetDpiForWindow` is only available on Windows 10 1607+. Load it dynamically
/// to avoid a static import, falling back to a GDI system-DPI query on older
/// systems (Windows 7).
fn get_dpi_for_window(hwnd: HWND) -> u32 {
    static GET_DPI_FOR_WINDOW: std::sync::OnceLock<Option<GetDpiForWindowFn>> =
        std::sync::OnceLock::new();

    if let Some(get_dpi_for_window) = *GET_DPI_FOR_WINDOW.get_or_init(load_get_dpi_for_window) {
        return unsafe { get_dpi_for_window(hwnd) };
    }

    unsafe {
        let hdc = GetDC(Some(hwnd));
        if hdc.0.is_null() {
            return windows::Win32::UI::WindowsAndMessaging::USER_DEFAULT_SCREEN_DPI;
        }
        let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX) as u32;
        ReleaseDC(Some(hwnd), hdc);
        dpi
    }
}

/// Dynamically resolve `GetDpiForWindow` from user32.dll.
fn load_get_dpi_for_window() -> Option<GetDpiForWindowFn> {
    let user32 = unsafe { LoadLibraryW(windows::core::w!("user32.dll")) }.ok()?;
    let proc = unsafe { GetProcAddress(user32, windows::core::s!("GetDpiForWindow")) }?;
    Some(unsafe { std::mem::transmute(proc) })
}

type GetDpiForMonitorFn =
    unsafe extern "system" fn(HMONITOR, MONITOR_DPI_TYPE, *mut u32, *mut u32) -> HRESULT;

/// `GetDpiForMonitor` is only available on Windows 8.1+. Load it dynamically
/// from shcore.dll to avoid a static import.
fn get_dpi_for_monitor(monitor: HMONITOR) -> Option<(u32, u32)> {
    static GET_DPI_FOR_MONITOR: std::sync::OnceLock<Option<GetDpiForMonitorFn>> =
        std::sync::OnceLock::new();

    let get_dpi_for_monitor = (*GET_DPI_FOR_MONITOR.get_or_init(load_get_dpi_for_monitor))?;
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let result = unsafe { get_dpi_for_monitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    (result == S_OK).then_some((dpi_x, dpi_y))
}

/// Dynamically resolve `GetDpiForMonitor` from shcore.dll.
fn load_get_dpi_for_monitor() -> Option<GetDpiForMonitorFn> {
    let shcore = unsafe { LoadLibraryW(windows::core::w!("shcore.dll")) }.ok()?;
    let proc = unsafe { GetProcAddress(shcore, windows::core::s!("GetDpiForMonitor")) }?;
    Some(unsafe { std::mem::transmute(proc) })
}

// Referred to: https://github.com/tauri-apps/tao/blob/ab792dbd6c5f0a708c818b20eaff1d9a7534c7c1/src/platform_impl/windows/dpi.rs#L50
pub fn get_scale_factor(target: &Target) -> f64 {
    const BASE_DPI: u32 = 96;

    let dpi = match target {
        Target::Window(window) => get_dpi_for_window(window.raw_handle),
        Target::Display(display) => match get_dpi_for_monitor(display.raw_handle) {
            Some((dpi_x, _)) => dpi_x,
            None => BASE_DPI,
        },
    };

    let scale_factor = dpi as f64 / BASE_DPI as f64;
    scale_factor as f64
}

pub fn get_target_dimensions(target: &Target) -> (u64, u64) {
    match target {
        Target::Window(window) => unsafe {
            let hwnd = window.raw_handle;

            // get width and height of the window
            let mut rect = RECT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            (width as u64, height as u64)
        },
        Target::Display(display) => {
            let monitor = Monitor::from_raw_hmonitor(display.raw_handle.0);

            (
                monitor.width().unwrap() as u64,
                monitor.height().unwrap() as u64,
            )
        }
    }
}
