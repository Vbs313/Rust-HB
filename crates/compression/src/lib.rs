//! 压缩模块
//!
//! 提供 LZMA 兼容解压缩功能。
//! 对应 C# 版的 Hearthbuddy.Compression.Lzma

/// 解压 LZMA 压缩的数据
pub fn decompress_lzma(_data: &[u8]) -> Result<Vec<u8>, CompressError> {
    // TODO: 实现 LZMA 解压（使用 lzma-rust crate 或原生实现）
    Err(CompressError::Unsupported)
}

/// 压缩为 LZMA 格式
pub fn compress_lzma(_data: &[u8]) -> Result<Vec<u8>, CompressError> {
    Err(CompressError::Unsupported)
}

/// GZip 解压（备选）
pub fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

#[derive(Debug, thiserror::Error)]
pub enum CompressError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported compression format")]
    Unsupported,
    #[error("Data corrupted")]
    Corrupted,
}
