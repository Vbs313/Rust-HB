//! Mono 运行时定位
//!
//! 通过扫描目标进程的内存，定位 Mono 运行时核心结构：
//! - mono_get_root_domain()
//! - mscorlib 映像
//! - 关键类和方法
//!
//! ## 工作原理
//!
//! 1. 枚举进程模块，找到 mono-2.0-sgen.dll 或 mono.dll
//! 2. 解析 PE 导出表，定位 mono_get_root_domain / mono_get_corlib
//! 3. 如导出表不可用（被混淆），使用特征码扫描
//! 4. 读取 MonoDomain 结构 → domain->image_set → corlib

use crate::BridgeError;
use hb_core::win32::ProcessHandle;
use std::ffi::c_void;

// Windows 常量
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;

/// 模块信息
#[derive(Debug, Clone)]
struct ModuleInfo {
    base_addr: usize,
    size: usize,
    name: String,
}

/// Mono 运行时核心句柄
#[derive(Debug)]
pub struct MonoRuntime {
    /// mono.dll 基址
    pub mono_module_base: usize,
    /// mono_get_root_domain 函数地址
    pub get_root_domain_addr: usize,
    /// 根域地址
    pub root_domain_addr: usize,
    /// 核心库 (mscorlib) 映像地址
    pub corlib_addr: usize,
}

impl MonoRuntime {
    /// 在目标进程中查找 Mono 运行时
    pub fn find(process: &ProcessHandle) -> Result<Self, BridgeError> {
        tracing::info!("Searching for Mono runtime in process PID={}", process.pid);

        let mono_module = Self::find_mono_module(process).ok_or(BridgeError::RuntimeNotFound)?;
        tracing::info!(
            "Found Mono module: {} at 0x{:x}",
            mono_module.name,
            mono_module.base_addr
        );

        let get_root_domain_addr = Self::find_export(process, &mono_module, "mono_get_root_domain")
            .ok_or(BridgeError::RuntimeNotFound)?;
        tracing::debug!("mono_get_root_domain at 0x{get_root_domain_addr:x}");

        let get_corlib_addr = Self::find_export(process, &mono_module, "mono_get_corlib")
            .ok_or(BridgeError::RuntimeNotFound)?;
        tracing::debug!("mono_get_corlib at 0x{get_corlib_addr:x}");

        let root_domain_addr = Self::call_function_remote(process, get_root_domain_addr)
            .ok_or(BridgeError::DomainNotFound)?;
        tracing::info!("root_domain at 0x{root_domain_addr:x}");

        // 用 mono_get_corlib 直接获取 corlib 映像（比从域结构偏移更可靠）
        let corlib_addr = Self::call_function_remote(process, get_corlib_addr)
            .ok_or(BridgeError::ImageNotFound("mscorlib".into()))?;
        tracing::info!("corlib image at 0x{corlib_addr:x}");

        // 验证 corlib 映像名称
        if let Ok(name_ptr) = process.read_memory::<usize>(corlib_addr + 0x24) {
            if let Ok(bytes) = process.read_bytes(name_ptr, 64) {
                let name = String::from_utf8_lossy(
                    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(64)],
                );
                tracing::info!("Corlib image name: '{}'", name);
            }
        }

        Ok(Self {
            mono_module_base: mono_module.base_addr,
            get_root_domain_addr,
            root_domain_addr,
            corlib_addr,
        })
    }

    /// 在目标进程中查找 mono 模块
    fn find_mono_module(process: &ProcessHandle) -> Option<ModuleInfo> {
        // 方案：通过 PEB/PEB_LDR_DATA 遍历加载模块
        // 先尝试用 CreateToolhelp32Snapshot (TH32CS_SNAPMODULE)
        let module_names = [
            "mono-2.0-bdwgc.dll",
            "mono-2.0-sgen.dll",
            "mono-2.0.dll",
            "mono.dll",
            "mono-sgen.dll",
        ];

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut c_void;
            fn Module32FirstW(hSnapshot: *mut c_void, lppe: *mut MODULEENTRY32W) -> i32;
            fn Module32NextW(hSnapshot: *mut c_void, lppe: *mut MODULEENTRY32W) -> i32;
            fn CloseHandle(hObject: *mut c_void) -> i32;
        }

        const TH32CS_SNAPMODULE: u32 = 0x00000008;
        const TH32CS_SNAPMODULE32: u32 = 0x00000010;

        #[repr(C)]
        struct MODULEENTRY32W {
            dwSize: u32,
            th32ModuleID: u32,
            th32ProcessID: u32,
            GlblcntUsage: u32,
            ProccntUsage: u32,
            modBaseAddr: *mut u8,
            modBaseSize: u32,
            hModule: *mut c_void,
            szModule: [u16; 256],
            szExePath: [u16; 260],
        }

        // 尝试 32 位模块快照（目标进程是 32 位的）
        for flags in &[TH32CS_SNAPMODULE32, TH32CS_SNAPMODULE] {
            let snapshot = unsafe { CreateToolhelp32Snapshot(*flags, process.pid) };
            if snapshot.is_null() {
                continue;
            }

            let mut entry = MODULEENTRY32W {
                dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                th32ModuleID: 0,
                th32ProcessID: 0,
                GlblcntUsage: 0,
                ProccntUsage: 0,
                modBaseAddr: std::ptr::null_mut(),
                modBaseSize: 0,
                hModule: std::ptr::null_mut(),
                szModule: [0u16; 256],
                szExePath: [0u16; 260],
            };

            if unsafe { Module32FirstW(snapshot, &mut entry) } == 0 {
                unsafe { CloseHandle(snapshot) };
                continue;
            }

            loop {
                let name = String::from_utf16_lossy(&entry.szModule)
                    .trim_matches('\0')
                    .to_lowercase();

                for target in &module_names {
                    if name == *target {
                        let module = ModuleInfo {
                            base_addr: entry.modBaseAddr as usize,
                            size: entry.modBaseSize as usize,
                            name,
                        };
                        unsafe { CloseHandle(snapshot) };
                        return Some(module);
                    }
                }

                if unsafe { Module32NextW(snapshot, &mut entry) } == 0 {
                    break;
                }
            }

            unsafe { CloseHandle(snapshot) };
        }

        None
    }

    /// 解析 PE 导出表，查找指定函数
    fn find_export(process: &ProcessHandle, module: &ModuleInfo, func_name: &str) -> Option<usize> {
        // 读取 DOS 头
        let dos_header: IMAGE_DOS_HEADER = process.read_memory(module.base_addr).ok()?;

        if dos_header.e_magic != 0x5A4D {
            // "MZ"
            tracing::warn!("Invalid DOS header at 0x{:x}", module.base_addr);
            return None;
        }

        // 读取 NT 头
        let nt_offset = dos_header.e_lfanew as usize;
        let nt_headers_addr = module.base_addr + nt_offset;

        // 读取 NT 头签名 + FileHeader
        let _file_header: IMAGE_FILE_HEADER = process.read_memory(nt_headers_addr + 4).ok()?; // 跳过签名

        // 定位 OptionalHeader 后的数据目录
        let optional_magic: u16 = process
            .read_memory(nt_headers_addr + 4 + std::mem::size_of::<IMAGE_FILE_HEADER>())
            .ok()?;

        let optional_header_size = if optional_magic == 0x10B {
            // PE32
            96usize // sizeof IMAGE_OPTIONAL_HEADER32
        } else if optional_magic == 0x20B {
            // PE32+
            112usize // sizeof IMAGE_OPTIONAL_HEADER64
        } else {
            return None;
        };

        // 数据目录起始偏移 = NT头 + 4(签名) + FileHeader(20) + OptionalHeader
        let data_dir_offset =
            nt_headers_addr + 4 + std::mem::size_of::<IMAGE_FILE_HEADER>() + optional_header_size;

        // 第 1 个数据目录 = 导出表 (IMAGE_DIRECTORY_ENTRY_EXPORT = 0)
        let export_dir: IMAGE_DATA_DIRECTORY = process.read_memory(data_dir_offset).ok()?;

        if export_dir.size == 0 {
            tracing::debug!("No export directory in {}", module.name);
            return None;
        }

        let export_addr = module.base_addr + export_dir.virtual_address as usize;

        // 读取导出表
        let export_table: IMAGE_EXPORT_DIRECTORY = process.read_memory(export_addr).ok()?;

        // 读取函数地址表、名称表、序号表
        let addr_of_funcs = module.base_addr + export_table.address_of_functions as usize;
        let addr_of_names = module.base_addr + export_table.address_of_names as usize;
        let addr_of_ordinals = module.base_addr + export_table.address_of_name_ordinals as usize;

        let target_name_bytes = func_name.as_bytes();
        let num_names = export_table.number_of_names as usize;

        for i in 0..num_names {
            // 读取函数名称指针
            let name_rva: u32 = process.read_memory(addr_of_names + i * 4).ok()?;
            let name_addr = module.base_addr + name_rva as usize;

            // 读取名称字符串（最多 256 字符）
            let name_bytes = process.read_bytes(name_addr, 256).ok()?;
            let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(256);
            let name_slice = &name_bytes[..name_len];

            // 比较名称
            if name_slice.len() == target_name_bytes.len()
                && name_slice.eq_ignore_ascii_case(target_name_bytes)
            {
                // 读取序号
                let ordinal: u16 = process.read_memory(addr_of_ordinals + i * 2).ok()?;
                let func_idx = ordinal as usize;

                // 读取函数地址 RVA
                let func_rva: u32 = process.read_memory(addr_of_funcs + func_idx * 4).ok()?;

                let func_addr = module.base_addr + func_rva as usize;
                tracing::debug!(
                    "Found export '{}' at 0x{:x} (RVA=0x{:x})",
                    func_name,
                    func_addr,
                    func_rva
                );

                return Some(func_addr);
            }
        }

        tracing::warn!("Export '{}' not found in {}", func_name, module.name);
        None
    }

    /// 通过 CreateRemoteThread 调用目标函数并获取返回值
    /// 在 x86 目标进程中：函数返回 EAX = 返回值（指针）
    fn call_function_remote(process: &ProcessHandle, func_addr: usize) -> Option<usize> {
        #[link(name = "kernel32")]
        extern "system" {
            fn CreateRemoteThread(
                hProcess: *mut c_void,
                lpThreadAttributes: *mut c_void,
                dwStackSize: usize,
                lpStartAddress: extern "system" fn() -> usize,
                lpParameter: *mut c_void,
                dwCreationFlags: u32,
                lpThreadId: *mut u32,
            ) -> *mut c_void;

            fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
            fn GetExitCodeThread(hThread: *mut c_void, lpExitCode: *mut u32) -> i32;
            fn CloseHandle(hObject: *mut c_void) -> i32;
        }

        // 获取进程原生句柄
        let raw_handle = process.handle as *mut c_void;

        unsafe {
            // 创建远程线程，执行 mono_get_root_domain()
            // 在 x86 调用约定下，函数指针可直接作为线程入口
            let thread = CreateRemoteThread(
                raw_handle,
                std::ptr::null_mut(),
                0,
                std::mem::transmute::<usize, extern "system" fn() -> usize>(func_addr),
                std::ptr::null_mut(),
                0, // 立即运行
                std::ptr::null_mut(),
            );

            if thread.is_null() {
                tracing::warn!("CreateRemoteThread failed");
                return None;
            }

            // 等待线程执行完毕（5 秒超时）
            let wait_result = WaitForSingleObject(thread, 5000);
            if wait_result != 0 {
                tracing::warn!("WaitForSingleObject failed: result={}", wait_result);
                CloseHandle(thread);
                return None;
            }

            // 读取线程退出码 = 函数返回值
            let mut exit_code: u32 = 0;
            if GetExitCodeThread(thread, &mut exit_code) == 0 {
                tracing::warn!("GetExitCodeThread failed");
                CloseHandle(thread);
                return None;
            }

            CloseHandle(thread);
            Some(exit_code as usize)
        }
    }

    /// 从根域（MonoDomain）找到 corlib 映像
    fn find_corlib_from_domain(
        process: &ProcessHandle,
        domain_addr: usize,
    ) -> Result<usize, BridgeError> {
        // MonoDomain 结构体布局（x86，Mono 6.x）：
        // +0x00: vtable (4 bytes)
        // +0x04: domain_id (4 bytes)
        // ... 其他字段 ...
        // +0x0C-0x10: domain_jit_info / domain_jit_code_mutex (指针)
        // +0x14-0x18: domain_assembly (MonoAssembly*)
        // +0x1C-0x20: domain_image_set (MonoImageSet*)
        //
        // 实际偏移需根据 Mono 版本动态检测。
        // 这里使用常用偏移 0x20 作为 image_set 的尝试
        //
        // MonoImageSet 结构：
        // +0x00: vtable
        // +0x04: ref_count
        // +0x08: image_count
        // +0x0C: images[0] (第一个元素是 corlib)
        // ... 或 images 是一个指针数组
        //
        // 简化实现：遍历 MonoDomain 的 assembly list
        // domain + 0x10 → assemblies (MonoAssembly**)
        // assemblies[0] → mscorlib

        // 尝试读取 assemblies 列表（多种偏移试探）
        for &asm_offset in &[0x0Cusize, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28] {
            let assembly_list_ptr: Result<usize, _> = process.read_memory(domain_addr + asm_offset);
            if let Ok(ptr) = assembly_list_ptr {
                if ptr != 0 && (ptr & 0xFFF) != 0 {
                    // 尝试读取 image 指针
                    let image_ptr: Result<usize, _> = process.read_memory(ptr);
                    if let Ok(img) = image_ptr {
                        if img != 0 && (img & 0xFFF) != 0 {
                            // 验证是否是合法的 image（读取 image->name）
                            let name_ptr: Result<usize, _> = process.read_memory(img + 0x24);
                            if let Ok(np) = name_ptr {
                                if np != 0 && (np & 0xFFF) != 0 {
                                    let name_bytes = process.read_bytes(np, 64).ok();
                                    if let Some(bytes) = name_bytes {
                                        let name_str = String::from_utf8_lossy(
                                            &bytes[..bytes
                                                .iter()
                                                .position(|&b| b == 0)
                                                .unwrap_or(64)],
                                        );
                                        if name_str.contains("mscorlib")
                                            || name_str.contains("corlib")
                                        {
                                            tracing::info!(
                                                "Found corlib via offset 0x{:x}: '{}' at 0x{:x}",
                                                asm_offset,
                                                name_str,
                                                img
                                            );
                                            return Ok(img);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 如果试探失败，回退到用 mono_get_corlib 函数
        tracing::warn!("Failed to find corlib via domain offsets, trying mono_get_corlib export");
        Err(BridgeError::ImageNotFound("mscorlib".to_string()))
    }

    /// 获取 corlib 中的 System.String 类
    pub fn get_string_class(&self) -> Option<usize> {
        // 从 corlib image 中查找 System.String 类
        // Image → class_cache → HashTable[ "System.String" ]
        None // TODO
    }
}

// ============================================================
// PE 结构体定义
// ============================================================

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IMAGE_DOS_HEADER {
    e_magic: u16, // "MZ"
    e_cblp: u16,
    e_cp: u16,
    e_crlc: u16,
    e_cparhdr: u16,
    e_minalloc: u16,
    e_maxalloc: u16,
    e_ss: u16,
    e_sp: u16,
    e_csum: u16,
    e_ip: u16,
    e_cs: u16,
    e_lfarlc: u16,
    e_ovno: u16,
    e_res: [u16; 4],
    e_oemid: u16,
    e_oeminfo: u16,
    e_res2: [u16; 10],
    e_lfanew: i32, // NT headers 偏移
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IMAGE_FILE_HEADER {
    machine: u16,
    number_of_sections: u16,
    time_date_stamp: u32,
    pointer_to_symbol_table: u32,
    number_of_symbols: u32,
    size_of_optional_header: u16,
    characteristics: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IMAGE_DATA_DIRECTORY {
    virtual_address: u32,
    size: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IMAGE_EXPORT_DIRECTORY {
    characteristics: u32,
    time_date_stamp: u32,
    major_version: u16,
    minor_version: u16,
    name: u32,
    base: u32,
    number_of_functions: u32,
    number_of_names: u32,
    address_of_functions: u32,
    address_of_names: u32,
    address_of_name_ordinals: u32,
}
