//! 窗口背景：直接调用 Win11 的 DWM 接口打开 / 关闭云母。
//!
//! 窗口没有系统标题栏（decorations: false），整个窗口都是客户区。
//! 系统背景（DWMWA_SYSTEMBACKDROP_TYPE）默认只画在非客户区，必须先用
//! DwmExtendFrameIntoClientArea 把边框扩展到整个窗口，云母才看得见。
//!
//! 扩展边框后，DWM 会在右上角画出系统自己的最小化 / 最大化 / 关闭按钮（和我们自己的按钮叠在一起）。
//! 这些按钮来自窗口的系统菜单样式 WS_SYSMENU，所以开着云母时把它去掉；
//! tao 改窗口状态（比如最小化）时会把样式写回去，所以窗口事件里会再去掉一次（见 `refresh`）。
//!
//! 注意：云母只取桌面“壁纸图片”的颜色。桌面背景设成“纯色”时没有图片可取，云母就只剩一层灰色；
//! 窗口不在前台时，系统也会把云母换成纯色。

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmGetWindowAttribute, DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_NONE,
        DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE,
    };
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, WS_SYSMENU,
    };

    /// 现在边框是不是扩展到了整个窗口（云母开着）
    static EXTENDED: AtomicBool = AtomicBool::new(false);

    fn set_kind(hwnd: HWND, kind: DWM_SYSTEMBACKDROP_TYPE) -> Result<(), String> {
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &kind as *const _ as _,
                std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
            )
        }
        .map_err(|e| format!("设置窗口背景失败：{e}"))
    }

    fn extend(hwnd: HWND, on: bool) -> Result<(), String> {
        let m = if on { -1 } else { 0 };
        let margins = MARGINS { cxLeftWidth: m, cxRightWidth: m, cyTopHeight: m, cyBottomHeight: m };
        unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) }.map_err(|e| format!("扩展窗口边框失败：{e}"))
    }

    /// 打开 / 去掉系统菜单样式（决定 DWM 画不画系统按钮）；样式没变就什么都不做
    fn sysmenu(hwnd: HWND, on: bool) {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let has = style & WS_SYSMENU.0 as isize != 0;
            if has == on {
                return;
            }
            let next = if on { style | WS_SYSMENU.0 as isize } else { style & !(WS_SYSMENU.0 as isize) };
            SetWindowLongPtrW(hwnd, GWL_STYLE, next);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// 只在状态真的变了时调用 DWM：每次调用都会让系统重画窗口边框，看起来像闪了一下
    pub fn apply(hwnd: HWND, mica: bool) -> Result<(), String> {
        let was = EXTENDED.load(Ordering::Relaxed);
        if mica {
            sysmenu(hwnd, false);
            if !was {
                extend(hwnd, true)?;
                EXTENDED.store(true, Ordering::Relaxed);
            }
            set_kind(hwnd, DWMSBT_MAINWINDOW)
        } else {
            if was {
                set_kind(hwnd, DWMSBT_NONE)?;
                extend(hwnd, false)?;
                EXTENDED.store(false, Ordering::Relaxed);
            }
            sysmenu(hwnd, true);
            Ok(())
        }
    }

    /// 窗口状态变化后（tao 可能把系统菜单样式写回去了）再去掉一次
    pub fn refresh(hwnd: HWND) {
        if EXTENDED.load(Ordering::Relaxed) {
            sysmenu(hwnd, false);
        }
    }

    /// 读回系统现在用的背景类型（2 = 云母），用来确认调用生效
    pub fn current(hwnd: HWND) -> Option<i32> {
        let mut v: i32 = 0;
        unsafe {
            DwmGetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &mut v as *mut i32 as _, std::mem::size_of::<i32>() as u32)
        }
        .ok()
        .map(|_| v)
    }
}

#[cfg(windows)]
fn hwnd_of(window: &tauri::Window) -> Result<windows::Win32::Foundation::HWND, String> {
    let h = window.hwnd().map_err(|e| e.to_string())?;
    Ok(windows::Win32::Foundation::HWND(h.0))
}

/// 打开（mica = true）或关闭云母；返回系统读回来的背景类型，2 表示云母
#[cfg(windows)]
pub fn set(window: &tauri::Window, mica: bool) -> Result<Option<i32>, String> {
    let hwnd = hwnd_of(window)?;
    imp::apply(hwnd, mica)?;
    Ok(imp::current(hwnd))
}

/// 窗口事件里调用：开着云母时，确保系统按钮不会冒出来
#[cfg(windows)]
pub fn refresh(window: &tauri::Window) {
    if let Ok(hwnd) = hwnd_of(window) {
        imp::refresh(hwnd);
    }
}

#[cfg(not(windows))]
pub fn set(_window: &tauri::Window, _mica: bool) -> Result<Option<i32>, String> {
    Ok(None)
}

#[cfg(not(windows))]
pub fn refresh(_window: &tauri::Window) {}
