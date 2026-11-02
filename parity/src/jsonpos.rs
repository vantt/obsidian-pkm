//! Tìm số dòng của từng giá trị trong một văn bản JSON, để báo lệch theo `file:dòng`.
//!
//! Chỉ quét cấu trúc (không dựng giá trị); văn bản đã được `serde_json` đọc thành công trước đó,
//! nên ở đây gặp chỗ sai cú pháp thì dừng và trả phần đã quét.

use std::collections::HashMap;

/// JSON pointer (RFC 6901, ví dụ `/items/3/node`) -> số dòng (từ 1) nơi giá trị bắt đầu.
pub fn value_lines(text: &str) -> HashMap<String, usize> {
    let mut scan = Scanner {
        bytes: text.as_bytes(),
        pos: 0,
        line: 1,
        out: HashMap::new(),
    };
    scan.value(String::new());
    scan.out
}

/// Dòng của `pointer`, hoặc của cha gần nhất có trong bảng (khoá bị thiếu ở một bên).
pub fn line_of(lines: &HashMap<String, usize>, pointer: &str) -> Option<usize> {
    let mut p = pointer;
    loop {
        if let Some(n) = lines.get(p) {
            return Some(*n);
        }
        {
            let i = p.rfind('/')?;
            p = &p[..i]
        }
    }
}

/// Một đoạn của JSON pointer: `~` thành `~0`, `/` thành `~1`.
pub fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: usize,
    out: HashMap<String, usize>,
}

impl Scanner<'_> {
    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b'\n' => self.line += 1,
                b' ' | b'\t' | b'\r' => {}
                _ => return,
            }
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn value(&mut self, path: String) -> Option<()> {
        self.skip_ws();
        self.out.insert(path.clone(), self.line);
        match self.peek()? {
            b'{' => self.object(&path),
            b'[' => self.array(&path),
            b'"' => self.string().map(|_| ()),
            _ => {
                while let Some(b) = self.peek() {
                    if matches!(b, b',' | b'}' | b']') || b.is_ascii_whitespace() {
                        break;
                    }
                    self.pos += 1;
                }
                Some(())
            }
        }
    }

    fn object(&mut self, path: &str) -> Option<()> {
        self.pos += 1;
        self.skip_ws();
        if self.peek()? == b'}' {
            self.pos += 1;
            return Some(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            if self.peek()? != b':' {
                return None;
            }
            self.pos += 1;
            self.value(format!("{path}/{}", escape(&key)))?;
            self.skip_ws();
            match self.peek()? {
                b',' => self.pos += 1,
                b'}' => {
                    self.pos += 1;
                    return Some(());
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self, path: &str) -> Option<()> {
        self.pos += 1;
        self.skip_ws();
        if self.peek()? == b']' {
            self.pos += 1;
            return Some(());
        }
        let mut i = 0usize;
        loop {
            self.value(format!("{path}/{i}"))?;
            i += 1;
            self.skip_ws();
            match self.peek()? {
                b',' => self.pos += 1,
                b']' => {
                    self.pos += 1;
                    return Some(());
                }
                _ => return None,
            }
        }
    }

    /// Đọc một chuỗi JSON (con trỏ ở dấu `"` mở), trả nội dung đã giải mã.
    fn string(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.peek()? {
                b'\\' => self.pos += 2,
                b'"' => {
                    self.pos += 1;
                    break;
                }
                _ => self.pos += 1,
            }
        }
        let raw = std::str::from_utf8(self.bytes.get(start..self.pos)?).ok()?;
        serde_json::from_str(raw).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_of_nested_values() {
        let text =
            "{\n  \"a\": 1,\n  \"b\": [\n    {\"c\": \"x/y\"},\n    2\n  ],\n  \"d~e\": null\n}\n";
        let lines = value_lines(text);
        assert_eq!(lines[""], 1);
        assert_eq!(lines["/a"], 2);
        assert_eq!(lines["/b"], 3);
        assert_eq!(lines["/b/0"], 4);
        assert_eq!(lines["/b/0/c"], 4);
        assert_eq!(lines["/b/1"], 5);
        assert_eq!(lines["/d~0e"], 7);
        assert_eq!(line_of(&lines, "/b/0/zz"), Some(4));
    }

    #[test]
    fn escaped_quotes_inside_strings() {
        let text = "{\"k\\\"\": \"v\\\\\",\n\"n\": 1}";
        let lines = value_lines(text);
        assert_eq!(lines["/k\""], 1);
        assert_eq!(lines["/n"], 2);
    }
}
