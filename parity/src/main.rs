//! `parity`: so dump của bản Rust (hoặc một dump Python khác) với dump mẫu. Xem `parity/README.md`.
//!
//! Mã thoát: 0 khi không có lệch chưa có tên, 1 khi có, 2 khi lỗi (đọc file, dump hỏng, sai vault).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use pkm_parity::compare::{Comparison, compare};
use pkm_parity::dump::Dump;
use pkm_parity::report::{self, exit_code};
use pkm_parity::{exceptions, fixture_args, golden_dir, pinned_spec_sha, repo_root, rust_side};

#[derive(Parser)]
#[command(
    name = "parity",
    about = "Bộ so khớp Rust với Python (docs/PRD.md mục 6)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// So hai thư mục dump: EXPECTED là dump mẫu (golden hoặc Python), ACTUAL là dump cần kiểm.
    Compare {
        expected: PathBuf,
        actual: PathBuf,
        #[command(flatten)]
        out: OutputArgs,
    },
    /// Chạy bản Rust trên VAULT theo bộ câu của một dump mẫu, ghi dump vào --out.
    RustDump {
        #[arg(long)]
        vault: PathBuf,
        /// Dump mẫu cho bộ câu và cách đọc vault (mặc định: golden của sha ghim).
        #[arg(long)]
        like: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// In tham số `npx` dựng vault fixture theo spec/spec.lock (CI thêm `--out DIR/<preset>`).
    FixtureArgs,
    /// Chạy bản Rust trên VAULT rồi so với dump mẫu (mặc định golden của sha ghim). CI dùng lệnh này.
    Check {
        #[arg(long)]
        vault: PathBuf,
        /// Dump mẫu (mặc định: parity/golden/<sha trong spec/spec.lock>).
        #[arg(long)]
        golden: Option<PathBuf>,
        /// Nơi ghi dump của bản Rust (mặc định: target/parity/rust-dump).
        #[arg(long)]
        dump_out: Option<PathBuf>,
        #[command(flatten)]
        out: OutputArgs,
    },
}

#[derive(Args)]
struct OutputArgs {
    /// File báo cáo JSON (mặc định: parity/report.json).
    #[arg(long)]
    report: Option<PathBuf>,
    /// Vault thật của user: chỉ in số dòng lệch theo bảng và theo tool, không in nội dung note.
    #[arg(long)]
    private: bool,
    /// Coi mục "chưa có" là lỗi (dùng khi mọi tầng của Rust đã xong).
    #[arg(long)]
    strict: bool,
    /// File ngoại lệ có tên (mặc định: parity/exceptions.toml).
    #[arg(long)]
    exceptions: Option<PathBuf>,
    /// Nhật ký chuyển, nơi mỗi ngoại lệ phải có mục "Lệch có chủ ý" (mặc định: spec/PORTING.md).
    #[arg(long)]
    porting: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("parity: lỗi: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn default_golden() -> Result<PathBuf> {
    Ok(golden_dir(&pinned_spec_sha()?))
}

fn run(cli: Cli) -> Result<u8> {
    match cli.cmd {
        Cmd::Compare {
            expected,
            actual,
            out,
        } => {
            let e = Dump::open(&expected)?;
            let a = Dump::open(&actual)?;
            finish(&e, &a, &out)
        }
        Cmd::RustDump { vault, like, out } => {
            let like = Dump::open(&like.map_or_else(default_golden, Ok)?)?;
            let m = rust_side::dump(&vault, &like, &out)?;
            println!(
                "đã ghi dump Rust vào {} ({} mục chưa có)",
                out.display(),
                m.not_available.len()
            );
            Ok(0)
        }
        Cmd::FixtureArgs => {
            println!("{}", fixture_args()?.join(" "));
            Ok(0)
        }
        Cmd::Check {
            vault,
            golden,
            dump_out,
            out,
        } => {
            let golden = golden.map_or_else(default_golden, Ok)?;
            let e = Dump::open(&golden)
                .with_context(|| format!("không mở được dump mẫu {}", golden.display()))?;
            let dir = dump_out
                .unwrap_or_else(|| repo_root().join("target").join("parity").join("rust-dump"));
            rust_side::dump(&vault, &e, &dir)?;
            let a = Dump::open(&dir)?;
            finish(&e, &a, &out)
        }
    }
}

fn finish(expected: &Dump, actual: &Dump, out: &OutputArgs) -> Result<u8> {
    let exc_path = out
        .exceptions
        .clone()
        .unwrap_or_else(|| repo_root().join("parity").join("exceptions.toml"));
    let porting = out
        .porting
        .clone()
        .unwrap_or_else(|| repo_root().join("spec").join("PORTING.md"));
    let exc = exceptions::load(&exc_path, &porting)?;
    let cmp: Comparison = compare(expected, actual, &exc)?;
    let ctx = report::Context {
        expected,
        actual,
        private: out.private,
        strict: out.strict,
    };
    let report_path = out
        .report
        .clone()
        .unwrap_or_else(|| repo_root().join("parity").join("report.json"));
    write_report(&report_path, &report::to_json(&cmp, &ctx))?;
    print!("{}", report::summary(&cmp, &ctx));
    println!("Báo cáo: {}", report_path.display());
    Ok(u8::try_from(exit_code(&cmp, out.strict)).unwrap_or(1))
}

fn write_report(path: &Path, v: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("không tạo được {}", parent.display()))?;
    }
    let mut text = serde_json::to_string_pretty(v)?;
    text.push('\n');
    std::fs::write(path, text).with_context(|| format!("không ghi được {}", path.display()))
}
