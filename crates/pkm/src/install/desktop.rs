//! Claude Desktop có đang chạy không. Claude Desktop giữ cấu hình trong bộ nhớ và ghi lại khi thoát, nên sửa file lúc
//! app đang chạy sẽ mất (như `app_running` của `desktop_config.py`).

use std::io;
use std::process::{Command, Stdio};

/// Hỏi hệ điều hành xem Claude Desktop có đang chạy không: `Some(tiến trình)` (đường dẫn exe khi biết, để người dùng
/// thấy vì sao bị coi là Claude Desktop), `None` khi không. Lỗi khi không hỏi được (thiếu công cụ hệ thống).
pub fn desktop_process() -> io::Result<Option<String>> {
    if cfg!(windows) {
        windows_process()
    } else if cfg!(target_os = "macos") {
        pgrep("Claude")
    } else {
        // Không có bản Linux chính thức; bản cộng đồng chạy dưới tên `claude-desktop`. Không tìm `claude` vì đó là
        // tên của Claude Code.
        pgrep("claude-desktop")
    }
}

/// `pgrep -x NAME`: mã 0 là có, 1 là không, khác là lỗi.
fn pgrep(name: &str) -> io::Result<Option<String>> {
    let status = Command::new("pgrep")
        .args(["-x", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    match status.code() {
        Some(0) => Ok(Some(format!("tiến trình `{name}`"))),
        Some(1) => Ok(None),
        _ => Err(io::Error::other(format!("pgrep -x {name} trả {status}"))),
    }
}

/// Windows: liệt kê đường dẫn của mọi `claude.exe` qua PowerShell để bỏ qua Claude Code (cũng tên `claude.exe`).
/// PowerShell không chạy được thì lùi về `tasklist` như Python (mọi `claude.exe` đều tính).
fn windows_process() -> io::Result<Option<String>> {
    let script = "Get-CimInstance Win32_Process -Filter \"Name='claude.exe'\" | ForEach-Object { \
                  if ($_.ExecutablePath) { $_.ExecutablePath } else { '?' } }";
    let ps = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    if let Ok(out) = ps
        && out.status.success()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        return Ok(first_desktop_exe(&text));
    }
    let out = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq claude.exe", "/NH"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!("tasklist trả {}", out.status)));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .to_lowercase()
        .contains("claude.exe")
        .then(|| "claude.exe (tasklist không cho biết đường dẫn)".to_owned()))
}

/// Dòng đầu tiên (mỗi dòng một đường dẫn exe) là Claude Desktop, theo [`is_desktop_exe`].
fn first_desktop_exe(lines: &str) -> Option<String> {
    lines
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && is_desktop_exe(l))
        .map(|l| {
            if l == "?" {
                "claude.exe (Windows không cho đọc đường dẫn)".to_owned()
            } else {
                l.to_owned()
            }
        })
}

/// Thư mục chỉ chứa Claude Code (cũng là `claude.exe`), so với đường dẫn viết thường, dấu `\`: cài native
/// (`~\.local\bin`), npm, winget (gói `Anthropic.ClaudeCode` và lối tắt trong `WinGet\Links`, chỉ có ở gói
/// portable), scoop (`claude-code`) và `%LOCALAPPDATA%\Programs` của bản cài theo người dùng.
const CLAUDE_CODE_DIRS: &[&str] = &[
    "\\.local\\bin\\",
    "\\node_modules\\",
    "\\npm\\",
    "\\microsoft\\winget\\links\\",
    "\\microsoft\\winget\\packages\\anthropic.claudecode",
    "\\scoop\\apps\\claude-code\\",
    "\\programs\\claude-code\\",
    "\\programs\\claudecode\\",
    "\\programs\\claude code\\",
];

/// Một `claude.exe` (đường dẫn đầy đủ, hoặc `?` khi Windows không cho đọc) có phải Claude Desktop không: mọi chỗ ngoài
/// [`CLAUDE_CODE_DIRS`] (gồm `AnthropicClaude\app-*`, gói MS Store `WindowsApps\Claude_*`, và đường dẫn không đọc
/// được) tính là Claude Desktop để không lỡ ghi đè cấu hình.
pub fn is_desktop_exe(path: &str) -> bool {
    let p = path.to_lowercase().replace('/', "\\");
    !CLAUDE_CODE_DIRS.iter().any(|d| p.contains(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_is_not_desktop() {
        assert!(!is_desktop_exe(r"C:\Users\u\.local\bin\claude.exe"));
        assert!(!is_desktop_exe(
            r"C:\Users\u\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code\claude.exe"
        ));
        assert!(!is_desktop_exe(
            r"C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe"
        ));
        assert!(!is_desktop_exe(
            r"C:\Users\u\AppData\Local\Microsoft\WinGet\Links\claude.exe"
        ));
        assert!(!is_desktop_exe(
            r"C:\Users\u\scoop\apps\claude-code\current\claude.exe"
        ));
        assert!(!is_desktop_exe(
            r"C:\Users\u\AppData\Local\Programs\claude-code\claude.exe"
        ));
    }

    #[test]
    fn desktop_paths_and_unknown_paths_count_as_desktop() {
        assert!(is_desktop_exe(
            r"C:\Users\u\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe"
        ));
        assert!(is_desktop_exe(
            r"C:\Program Files\WindowsApps\Claude_1.0.0.0_x64__pzs8sxrjxfjjc\app\claude.exe"
        ));
        assert!(is_desktop_exe(
            r"C:\Users\u\AppData\Local\Programs\Claude\claude.exe"
        ));
        assert!(is_desktop_exe(
            r"C:\Users\u\scoop\apps\claude\current\claude.exe"
        ));
        assert!(is_desktop_exe("?"));
    }

    #[test]
    fn reports_first_desktop_path_and_skips_claude_code() {
        let ps = "C:\\Users\\u\\.local\\bin\\claude.exe\r\n\r\n\
                  C:\\Users\\u\\AppData\\Local\\AnthropicClaude\\app-1.0.0\\claude.exe\r\n";
        assert_eq!(
            first_desktop_exe(ps).as_deref(),
            Some(r"C:\Users\u\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe")
        );
        assert_eq!(
            first_desktop_exe("C:\\Users\\u\\.local\\bin\\claude.exe\n"),
            None
        );
        assert!(first_desktop_exe("?\n").unwrap().contains("không cho đọc"));
    }
}
