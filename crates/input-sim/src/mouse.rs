//! 鼠标操作封装
//!
//! 基于 Windows SendInput API 实现（raw FFI）

use crate::bezier::Point;
use crate::ffi::{SendInput, INPUT, INPUT_UNION, MOUSEINPUT};
use crate::InputError;

#[repr(C)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
}

const INPUT_MOUSE: u32 = 0;
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;

/// 获取当前鼠标位置
pub fn get_cursor_pos() -> Result<(i32, i32), InputError> {
    let mut pt = POINT { x: 0, y: 0 };
    let ret = unsafe { GetCursorPos(&mut pt) };
    if ret == 0 {
        return Err(InputError::CursorPos);
    }
    Ok((pt.x, pt.y))
}

/// 左键按下
pub fn hold_left() -> Result<(), InputError> {
    send_mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0)
}

/// 左键释放
pub fn release_left() -> Result<(), InputError> {
    send_mouse_event(MOUSEEVENTF_LEFTUP, 0, 0)
}

/// 左键点击
pub fn click_left() -> Result<(), InputError> {
    hold_left()?;
    std::thread::sleep(std::time::Duration::from_millis(
        30 + rand::random::<u64>() % 50,
    ));
    release_left()
}

/// 右键点击
pub fn click_right() -> Result<(), InputError> {
    send_mouse_event(MOUSEEVENTF_RIGHTDOWN, 0, 0)?;
    std::thread::sleep(std::time::Duration::from_millis(
        30 + rand::random::<u64>() % 50,
    ));
    send_mouse_event(MOUSEEVENTF_RIGHTUP, 0, 0)
}

/// 沿路径移动鼠标
pub fn move_along_path(points: &[Point], delay_ms: u64) -> Result<(), InputError> {
    let screen_w = 65536u32;

    for point in points {
        let abs_x = ((point.x / 1920.0) * screen_w as f64) as u32;
        let abs_y = ((point.y / 1080.0) * screen_w as f64) as u32;

        send_mouse_event(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE, abs_x, abs_y)?;

        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }
    Ok(())
}

/// 发送鼠标输入事件
fn send_mouse_event(flags: u32, dx: u32, dy: u32) -> Result<(), InputError> {
    let mi = MOUSEINPUT {
        dx: dx as i32,
        dy: dy as i32,
        mouseData: 0,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };

    let input = INPUT {
        type_: INPUT_MOUSE,
        u: INPUT_UNION {
            mi: std::mem::ManuallyDrop::new(mi),
        },
    };

    unsafe {
        let sent = SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            return Err(InputError::SimFailed("SendInput returned 0".into()));
        }
    }
    Ok(())
}
