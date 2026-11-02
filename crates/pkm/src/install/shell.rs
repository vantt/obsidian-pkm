//! Dòng lệnh chép chạy được (trường `fix` của `doctor --json`, lệnh gợi ý của bộ cài) cho shell mặc định của nền tảng:
//! PowerShell trên Windows (cùng shell với `install.ps1`), `sh` trên Unix. Đối số chỉ có ký tự an toàn thì để trần,
//! còn lại đặt trong nháy đơn (cả hai shell coi nội dung nháy đơn là chữ, không mở rộng `$` hay `\`).

/// `exe` và `args` thành một dòng lệnh cho shell mặc định của máy này.
pub fn command_line<S: AsRef<str>>(exe: &str, args: &[S]) -> String {
    if cfg!(windows) {
        powershell(exe, args)
    } else {
        posix(exe, args)
    }
}

/// Dòng lệnh cho `sh`.
pub fn posix<S: AsRef<str>>(exe: &str, args: &[S]) -> String {
    std::iter::once(exe)
        .chain(args.iter().map(AsRef::as_ref))
        .map(|a| quote(a, |c| "-_./:=,+@%".contains(c), "'\\''"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Dòng lệnh cho PowerShell: `& <exe> <args>` (toán tử gọi cần khi đường dẫn exe nằm trong nháy).
pub fn powershell<S: AsRef<str>>(exe: &str, args: &[S]) -> String {
    // `,` (mảng), `@` (splat), `$`, `;`, `` ` `` có nghĩa riêng trong PowerShell nên không để trần.
    let q = |a: &str| quote(a, |c| "-_./\\:".contains(c), "''");
    std::iter::once(format!("& {}", q(exe)))
        .chain(args.iter().map(|a| q(a.as_ref())))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `s` trần khi mọi ký tự là chữ số, chữ ASCII hoặc `extra`; không thì trong nháy đơn, nháy đơn bên trong thay bằng
/// `escaped_quote`.
fn quote(s: &str, extra: impl Fn(char) -> bool, escaped_quote: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || extra(c)) {
        return s.to_owned();
    }
    format!("'{}'", s.replace('\'', escaped_quote))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_quotes_only_what_needs_it() {
        assert_eq!(
            posix(
                "/home/u/.local/bin/pkm",
                &["mcp", "install", "/v/Kho ghi chú", "--db", ""]
            ),
            "/home/u/.local/bin/pkm mcp install '/v/Kho ghi chú' --db ''"
        );
        assert_eq!(posix("pkm", &["it's"]), r"pkm 'it'\''s'");
        assert_eq!(posix("pkm", &["$HOME", "a\\b"]), r"pkm '$HOME' 'a\b'");
    }

    #[test]
    fn powershell_uses_call_operator_and_literal_quotes() {
        assert_eq!(
            powershell(
                r"C:\Users\An\AppData\Local\pkm\bin\pkm.exe",
                &["doctor", "--json"]
            ),
            r"& C:\Users\An\AppData\Local\pkm\bin\pkm.exe doctor --json"
        );
        assert_eq!(
            powershell(
                r"C:\Program Files\pkm.exe",
                &[r"D:\Ghi chú", "capture,people", "it's", "$x"]
            ),
            r"& 'C:\Program Files\pkm.exe' 'D:\Ghi chú' 'capture,people' 'it''s' '$x'"
        );
    }
}
