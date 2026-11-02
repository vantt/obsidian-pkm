//! Binary `pkm`: CLI và MCP server trên lõi `pkm-core` (docs/PRD.md mục 10).

mod cli;
mod mcp;

use clap::Parser;

/// Chuỗi `pkm --version` in ra: phiên bản crate và sha đầy đủ của bản Python đặc tả mà
/// binary này khớp (`PKM_SPEC_SHA`, build.rs đọc từ `spec/spec.lock`).
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (spec ",
    env!("PKM_SPEC_SHA"),
    ")"
);

#[derive(Parser)]
#[command(name = "pkm", version = VERSION, about = "Index graph của vault Obsidian, bộ tool hỏi graph và MCP server")]
struct Args {}

fn main() {
    let _args = Args::parse();
}
