//! Binary `pkm`: CLI và MCP server trên lõi `pkm-core` (docs/PRD.md mục 10).

mod cli;
mod doctor;
mod install;
mod mcp;
mod setup;

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run(std::env::args_os().skip(1).collect())
}
