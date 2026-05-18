//! Mono 运行时类实例读取
//!
//! 对应 C# 版的 Triton.Game.Mono 模块。
//! 提供以下核心能力：
//! - 读取对象的字段值（值类型/引用类型/字符串/数组）
//! - 调用对象的方法
//! - 遍历继承链

use hb_core::win32::ProcessHandle;

/// Mono 对象包装
#[derive(Debug, Clone)]
pub struct MonoObject {
    /// 目标进程中的对象地址
    pub address: usize,
    /// vtable 表地址（指向 MonoClass）
    pub vtable_addr: usize,
    /// MonoClass 地址
    pub class_addr: usize,
}

/// 字段读取器
pub struct FieldReader<'a> {
    process: &'a ProcessHandle,
    /// 对象基址
    base_addr: usize,
}

impl<'a> FieldReader<'a> {
    pub fn new(process: &'a ProcessHandle, base_addr: usize) -> Self {
        Self { process, base_addr }
    }

    /// 读取 i4 (int32) 字段
    pub fn read_i32(&self, offset: usize) -> Result<i32, hb_core::error::Error> {
        self.process.read_memory::<i32>(self.base_addr + offset)
    }

    /// 读取 i8 (bool) 字段
    pub fn read_bool(&self, offset: usize) -> Result<bool, hb_core::error::Error> {
        let val: i8 = self.process.read_memory(self.base_addr + offset)?;
        Ok(val != 0)
    }

    /// 读取 r4 (float32) 字段
    pub fn read_f32(&self, offset: usize) -> Result<f32, hb_core::error::Error> {
        self.process.read_memory::<f32>(self.base_addr + offset)
    }

    /// 读取字符串字段（MonoString 指针）
    pub fn read_string(&self, offset: usize) -> Result<String, hb_core::error::Error> {
        let ptr: usize = self.process.read_memory(self.base_addr + offset)?;
        if ptr == 0 {
            return Ok(String::new());
        }
        // MonoString 结构: [length: i32] [chars: u16[]]
        let length: i32 = self.process.read_memory(ptr + 0x8)?;
        if length <= 0 || length > 1024 {
            return Ok(String::new());
        }
        let chars = self.process.read_bytes(ptr + 0xC, (length as usize) * 2)?;
        let chars_u16: Vec<u16> = chars
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&chars_u16))
    }

    /// 读取对象引用字段（返回 MonoObject）
    pub fn read_object(&self, offset: usize) -> Result<Option<MonoObject>, hb_core::error::Error> {
        let ptr: usize = self.process.read_memory(self.base_addr + offset)?;
        if ptr == 0 {
            return Ok(None);
        }
        let vtable: usize = self.process.read_memory(ptr)?;
        let class_addr = vtable.wrapping_sub(0); // mono vtable → class 偏移
        Ok(Some(MonoObject {
            address: ptr,
            vtable_addr: vtable,
            class_addr,
        }))
    }

    /// 读取枚举字段（按 i4 读取）
    pub fn read_enum<T: From<i32>>(&self, offset: usize) -> Result<T, hb_core::error::Error> {
        let val: i32 = self.read_i32(offset)?;
        Ok(T::from(val))
    }
}

/// 从 Mono 对象数组读取元素
pub fn read_mono_array<T>(
    process: &ProcessHandle,
    array_addr: usize,
    element_size: usize,
) -> Result<Vec<T>, hb_core::error::Error>
where
    T: Copy + Default,
{
    // MonoArray 结构: [vtable: ptr] [length: i32] [elements: T[]]
    let length: i32 = process.read_memory(array_addr + 0x8)?;
    if length <= 0 || length > 10000 {
        return Ok(Vec::new());
    }

    let mut result = Vec::with_capacity(length as usize);
    let data_start = array_addr + 0xC;

    for i in 0..(length as usize) {
        let elem: T = process.read_memory(data_start + i * element_size)?;
        result.push(elem);
    }

    Ok(result)
}
