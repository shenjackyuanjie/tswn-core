//! CLI 与桌面工具共用的文本文件读取规则。

use std::fs;
use std::io;
use std::path::Path;

/// 读取完整文件，遇到跳过的行时将文件路径和行数写到 stderr。
pub fn read_text_file(path: &Path) -> io::Result<String> {
    let (text, skipped) = decode_utf8_lines(fs::read(path)?);
    if skipped > 0 {
        eprintln!("{}：已跳过 {skipped} 行非 UTF-8 文本。", path.display());
    }
    Ok(text)
}

/// 去掉开头的 UTF-8 BOM，整行跳过无效 UTF-8，并返回文本和跳过的行数。
/// 合法行保留原始字节，包括空行、尾部换行和 CRLF。
pub fn decode_utf8_lines(mut bytes: Vec<u8>) -> (String, usize) {
    const BOM: &[u8] = b"\xef\xbb\xbf";
    if bytes.starts_with(BOM) {
        bytes.drain(..BOM.len());
    }
    let error = match String::from_utf8(bytes) {
        Ok(text) => return (text, 0),
        Err(error) => error,
    };
    let bytes = error.as_bytes();
    let mut text = String::with_capacity(bytes.len());
    let mut skipped = 0;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        match std::str::from_utf8(line) {
            Ok(line) => text.push_str(line),
            Err(_) => skipped += 1,
        }
    }
    (text, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_skips_entire_invalid_lines_and_keeps_valid_bytes() {
        let bytes = b"\xef\xbb\xbf\xe4\xb8\xad\xe6\x96\x87@team\r\ninvalid\xc8\xba\xcc\xe5\r\n\r\nmario\nlast\xed";
        assert_eq!(decode_utf8_lines(bytes.to_vec()), ("中文@team\r\n\r\nmario\n".to_owned(), 2));
        assert_eq!(decode_utf8_lines(b"\xff\n".to_vec()), (String::new(), 1));
        assert_eq!(decode_utf8_lines(b"\xef\xbb\xbfmario".to_vec()), ("mario".to_owned(), 0));
    }
}
