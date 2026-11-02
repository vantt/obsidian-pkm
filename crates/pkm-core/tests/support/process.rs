//! Kiểm và giết tiến trình theo PID trong test, trên Unix và Windows.

use std::process::{Command, Stdio};

/// Tiến trình `pid` còn chạy (đã được reap thì không còn).
#[cfg(unix)]
pub fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .expect("chạy kill -0")
        .success()
}

#[cfg(windows)]
pub fn alive(pid: u32) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stderr(Stdio::null())
        .output()
        .expect("chạy tasklist");
    String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
}

/// Giết `pid` ngay, không cho dọn dẹp (như khi tiến trình bị hệ điều hành giết).
#[allow(dead_code)] // `ohs_real` chỉ dùng `alive`.
pub fn kill_hard(pid: u32) {
    let status = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .status()
    } else {
        Command::new("kill").args(["-9", &pid.to_string()]).status()
    };
    assert!(
        status.expect("chạy lệnh kill").success(),
        "không giết được {pid}"
    );
}
