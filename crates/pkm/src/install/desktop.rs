//! Claude Desktop có đang chạy không. Claude Desktop giữ cấu hình trong bộ nhớ và ghi lại khi thoát, nên sửa file lúc
//! app đang chạy sẽ mất (như `app_running` của `desktop_config.py`).

use std::io;
use std::process::{Command, Stdio};

/// Hỏi hệ điều hành xem Claude Desktop có đang chạy không. Lỗi khi không hỏi được (thiếu công cụ hệ thống).
pub fn desktop_running() -> io::Result<bool> {
    if cfg!(windows) {
        windows_running()
    } else if cfg!(target_os = "macos") {
        pgrep("Claude")
    } else {
        // Không có bản Linux chính thức; bản cộng đồng chạy dưới tên `claude-desktop`. Không tìm `claude` vì đó là
        // tên của Claude Code.
        pgrep("claude-desktop")
    }
}

/// `pgrep -x NAME`: mã 0 là có, 1 là không, khác là lỗi.
fn pgrep(name: &str) -> io::Result<bool> {
    let status = Command::new("pgrep")
        .args(["-x", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(io::Error::other(format!("pgrep -x {name} trả {status}"))),
    }
}

/// Windows: liệt kê đường dẫn của mọi `claude.exe` qua PowerShell để bỏ qua Claude Code (cũng tên `claude.exe`).
/// PowerShell không chạy được thì lùi về `tasklist` như Python (mọi `claude.exe` đều tính).
fn windows_running() -> io::Result<bool> {
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
        return Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .any(is_desktop_exe));
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
        .contains("claude.exe"))
}

/// Một `claude.exe` (đường dẫn đầy đủ, hoặc `?` khi Windows không cho đọc) có phải Claude Desktop không. Claude Code
/// cài vào `~\.local\bin` hoặc qua npm; mọi chỗ khác (gồm đường dẫn không đọc được) tính là Claude Desktop để không
/// lỡ ghi đè cấu hình.
pub fn is_desktop_exe(path: &str) -> bool {
    let p = path.to_lowercase().replace('/', "\\");
    !(p.contains("\\.local\\bin\\") || p.contains("\\node_modules\\") || p.contains("\\npm\\"))
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
    }

    #[test]
    fn desktop_paths_and_unknown_paths_count_as_desktop() {
        assert!(is_desktop_exe(
            r"C:\Users\u\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe"
        ));
        assert!(is_desktop_exe(
            r"C:\Program Files\WindowsApps\Claude_1.0.0.0_x64__pzs8sxrjxfjjc\app\claude.exe"
        ));
        assert!(is_desktop_exe("?"));
    }
}
