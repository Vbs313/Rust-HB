//! # hb-process-mgr
//!
//! 进程管理模块
//!
//! 职责：
//! - 发现和附加炉石传说进程
//! - 多实例管理（支持多个炉石窗口）
//! - 窗口状态管理（激活、最小化、调整大小）
//! - 进程健康监控

use hb_core::win32::ProcessHandle;
use std::ffi::c_void;

// ============================================================
// Win32 FFI
// ============================================================

type HWND = *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(
        lpEnumFunc: Option<unsafe extern "system" fn(HWND, isize) -> i32>,
        lParam: isize,
    ) -> i32;

    fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;

    fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;

    fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> i32;

    fn SetForegroundWindow(hWnd: HWND) -> i32;

    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;

    fn MoveWindow(hWnd: HWND, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: i32) -> i32;
}

#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

const SW_RESTORE: i32 = 9;

// ============================================================
// 数据结构
// ============================================================

/// 炉石窗口信息
#[derive(Debug, Clone)]
pub struct HearthstoneWindow {
    pub pid: u32,
    pub hwnd: HWND,
    pub title: String,
    pub rect: Option<Rect>,
}

/// 窗口矩形
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

// ============================================================
// 进程管理器
// ============================================================

/// 进程管理器
pub struct ProcessManager {
    windows: Vec<HearthstoneWindow>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }

    /// 发现所有炉石窗口
    ///
    /// # Safety
    /// 内部使用 Win32 FFI 枚举窗口
    pub unsafe fn discover_windows(&mut self) -> Result<Vec<HearthstoneWindow>, ProcessError> {
        let mut windows: Vec<HearthstoneWindow> = Vec::new();
        let data = &mut windows as *mut Vec<HearthstoneWindow> as isize;

        let success = EnumWindows(Some(enum_window_callback), data);
        if success == 0 {
            return Err(ProcessError::EnumFailed("EnumWindows returned 0".to_string()));
        }

        self.windows = windows.clone();
        Ok(windows)
    }

    /// 附加到第一个炉石进程
    pub fn attach_first(&self) -> Result<ProcessHandle, ProcessError> {
        if self.windows.is_empty() {
            return Err(ProcessError::NoWindowFound);
        }
        ProcessHandle::find_by_name("Hearthstone")
            .map_err(|e| ProcessError::AttachFailed(e.to_string()))
    }

    /// 激活窗口
    ///
    /// # Safety
    /// `hwnd` 必须是有效的窗口句柄
    pub unsafe fn focus_window(&self, hwnd: HWND) -> Result<(), ProcessError> {
        if SetForegroundWindow(hwnd) == 0 {
            return Err(ProcessError::WindowError("SetForegroundWindow failed".to_string()));
        }
        ShowWindow(hwnd, SW_RESTORE);
        Ok(())
    }

    /// 调整窗口大小
    ///
    /// # Safety
    /// `hwnd` 必须是有效的窗口句柄
    pub unsafe fn resize_window(&self, hwnd: HWND, width: i32, height: i32) -> Result<(), ProcessError> {
        if MoveWindow(hwnd, 0, 0, width, height, 1) == 0 {
            return Err(ProcessError::WindowError("MoveWindow failed".to_string()));
        }
        Ok(())
    }
}

// ============================================================
// EnumWindows 回调
// ============================================================

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: isize) -> i32 {
    let windows = &mut *(lparam as *mut Vec<HearthstoneWindow>);

    let mut buf = [0u16; 256];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 256);
    if len == 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&buf[..len as usize]);

    if title.contains("炉石传说") || title.contains("Hearthstone") {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let has_rect = GetWindowRect(hwnd, &mut rect) != 0;

        windows.push(HearthstoneWindow {
            pid,
            hwnd,
            title,
            rect: if has_rect {
                Some(Rect {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: rect.bottom,
                })
            } else {
                None
            },
        });
    }

    1
}

// ============================================================
// 错误类型
// ============================================================

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("No Hearthstone window found")]
    NoWindowFound,
    #[error("Failed to enumerate windows: {0}")]
    EnumFailed(String),
    #[error("Failed to attach to process: {0}")]
    AttachFailed(String),
    #[error("Window operation failed: {0}")]
    WindowError(String),
}

unsafe impl Send for ProcessManager {}
