//! MCP stdio qua crate `rmcp` (tokio, một luồng). Cùng tool, cùng câu chữ với `s2-hand`.
//!
//!   s2-rmcp VAULT

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig,
    Tool, ToolsCapability,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use s2_mcp::{PROTOCOLS, SERVER_NAME, SERVER_VERSION, call_tool, instructions, tool_defs};
use std::borrow::Cow;
use std::path::PathBuf;
use std::process::ExitCode;

struct Probe {
    vault: PathBuf,
    tools: Vec<Tool>,
    versions: Vec<ProtocolVersion>,
}

impl ServerHandler for Probe {
    fn get_info(&self) -> ServerConfig {
        let mut tools = ToolsCapability::default();
        tools.list_changed = Some(false);
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(tools);
        let mut info = ServerConfig::new(caps).with_instructions(instructions(&self.vault));
        info.protocol_version = ProtocolVersion::V_2025_06_18;
        info.server_info = Implementation::new(SERVER_NAME, SERVER_VERSION);
        info
    }

    /// Thu hẹp về đúng ba bản Python nhận, để `rmcp` không đồng ý một bản mới hơn Python.
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(self.versions.clone())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request.arguments.unwrap_or_default();
        let (text, is_error) = call_tool(&self.vault, &request.name, &args);
        let content = vec![ContentBlock::text(text)];
        Ok(if is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        }
        .into())
    }
}

fn main() -> ExitCode {
    let Some(vault) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: s2-rmcp VAULT");
        return ExitCode::from(2);
    };
    if !vault.is_dir() {
        eprintln!("s2-rmcp: not a directory: {}", vault.display());
        return ExitCode::from(2);
    }
    let tools: Vec<Tool> = match serde_json::from_value(tool_defs()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("s2-rmcp: tool definitions do not fit rmcp::model::Tool: {e}");
            return ExitCode::FAILURE;
        }
    };
    let versions: Vec<ProtocolVersion> = ProtocolVersion::KNOWN_VERSIONS
        .iter()
        .filter(|v| PROTOCOLS.contains(&v.as_str()))
        .cloned()
        .collect();
    if versions.len() != PROTOCOLS.len() {
        eprintln!("s2-rmcp: rmcp does not know every protocol version in {PROTOCOLS:?}");
        return ExitCode::FAILURE;
    }
    let probe = Probe {
        vault,
        tools,
        versions,
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("s2-rmcp: cannot start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        let service = probe.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("s2-rmcp: {e}");
            ExitCode::FAILURE
        }
    }
}
