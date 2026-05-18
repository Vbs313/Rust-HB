//! # hb-input-sim
//!
//! 输入模拟模块
//!
//! 替代 C# 版的 Triton.Client 输入系统。
//! 提供人类化的鼠标和键盘操作，用于自动操作炉石传说 UI。
//!
//! ## 核心功能
//!
//! - 贝塞尔曲线鼠标移动（模拟人类手部轨迹）
//! - 鼠标点击（左键/右键/拖拽）
//! - 键盘输入
//! - 坐标系统校准

#![allow(non_snake_case, dead_code)]

pub mod bezier;
pub mod ffi;
pub mod keyboard;
pub mod mouse;

use hb_core::config::MouseSpeedMode;

/// 输入模拟器
pub struct InputSimulator {
    /// 鼠标速度模式
    pub speed_mode: MouseSpeedMode,
    /// 屏幕分辨率
    pub screen_width: i32,
    pub screen_height: i32,
}

impl InputSimulator {
    pub fn new(speed_mode: MouseSpeedMode) -> Self {
        // 获取屏幕分辨率
        let (w, h) = get_screen_resolution().unwrap_or((1920, 1080));
        Self {
            speed_mode,
            screen_width: w,
            screen_height: h,
        }
    }

    /// 在当前鼠标位置点击左键
    pub fn click_left(&self) -> Result<(), InputError> {
        mouse::click_left()
    }

    /// 在当前鼠标位置点击右键
    pub fn click_right(&self) -> Result<(), InputError> {
        mouse::click_right()
    }

    /// 移动到目标位置并点击左键
    pub fn move_and_click(&self, x: i32, y: i32) -> Result<(), InputError> {
        self.move_mouse(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(self.get_click_delay()));
        self.click_left()
    }

    /// 贝塞尔曲线移动到目标位置
    pub fn move_mouse(&self, target_x: i32, target_y: i32) -> Result<(), InputError> {
        let (start_x, start_y) = mouse::get_cursor_pos()?;
        let points = bezier::generate_bezier_path(start_x, start_y, target_x, target_y);
        mouse::move_along_path(&points, self.get_move_delay())
    }

    /// 拖拽操作
    pub fn drag(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<(), InputError> {
        self.move_mouse(from_x, from_y)?;
        mouse::hold_left()?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.move_mouse(to_x, to_y)?;
        mouse::release_left()
    }

    /// 获取移动延迟（毫秒）
    fn get_move_delay(&self) -> u64 {
        match self.speed_mode {
            MouseSpeedMode::Instant => 0,
            MouseSpeedMode::HumanLike => 5 + rand::random::<u64>() % 10,
            MouseSpeedMode::Custom {
                min_delay_ms,
                max_delay_ms,
            } => min_delay_ms + rand::random::<u64>() % (max_delay_ms - min_delay_ms + 1),
        }
    }

    /// 获取点击延迟
    fn get_click_delay(&self) -> u64 {
        match self.speed_mode {
            MouseSpeedMode::Instant => 0,
            MouseSpeedMode::HumanLike => 50 + rand::random::<u64>() % 150,
            MouseSpeedMode::Custom { .. } => 50,
        }
    }
}

/// 获取屏幕分辨率（raw FFI）
fn get_screen_resolution() -> Option<(i32, i32)> {
    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hWnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn ReleaseDC(hWnd: *mut std::ffi::c_void, hDC: *mut std::ffi::c_void) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
    }

    const HORZRES: i32 = 8;
    const VERTRES: i32 = 10;

    unsafe {
        let dc = GetDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let w = GetDeviceCaps(dc, HORZRES);
        let h = GetDeviceCaps(dc, VERTRES);
        ReleaseDC(std::ptr::null_mut(), dc);
        Some((w, h))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("Failed to get cursor position")]
    CursorPos,
    #[error("Input simulation failed: {0}")]
    SimFailed(String),
}
