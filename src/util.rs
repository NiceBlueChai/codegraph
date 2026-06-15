use std::fs;
use std::path::Path;

/// Read file content with automatic encoding detection.
/// Tries UTF-8 first, then GBK (common for Chinese Windows C++ projects),
/// falls back to lossy UTF-8.
pub fn read_file_content(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;

    // 1. UTF-8 (covers 99% of source files)
    if let Ok(s) = String::from_utf8(bytes.clone()) {
        return Ok(s);
    }

    // 2. GBK/GB2312 (common on Chinese Windows for C++ projects)
    let (decoded, _encoding, had_errors) = encoding_rs::GBK.decode(&bytes);
    if !had_errors {
        return Ok(decoded.into_owned());
    }

    // 3. Fallback: lossy UTF-8 (last resort, won't break tree-sitter)
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
