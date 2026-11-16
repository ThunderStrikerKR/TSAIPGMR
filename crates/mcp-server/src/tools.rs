use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    auth::{Role, Scope},
    store::{Receipt, Store, WriteOutcome},
};

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Lookup {
    #[schemars(length(min = 1, max = 64))]
    project_id: String,
    #[schemars(length(min = 1, max = 64))]
    request_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Write {
    #[schemars(length(min = 1, max = 64))]
    project_id: String,
    #[schemars(length(min = 1, max = 64))]
    request_id: String,
    #[schemars(
        length(min = 11, max = 4096),
        description = "Synthetic fixture text starting with [SYNTHETIC]. Maximum 4096 UTF-8 bytes."
    )]
    result: String,
}

#[derive(Clone)]
pub struct SyntheticTools {
    store: Store,
}

#[tool_handler]
impl ServerHandler for SyntheticTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "synthetic-mcp-server",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Use only synthetic fixtures. Obtain your project scope with server_status.",
            )
    }
}

#[tool_router]
impl SyntheticTools {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    #[tool(
        description = "Show the authenticated synthetic test project and role.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn server_status(&self, scope: Scope) -> CallToolResult {
        CallToolResult::structured(json!({
            "status": "ready",
            "project_id": scope.project.as_ref(),
            "role": scope.role.as_str(),
        }))
    }

    #[tool(
        description = "Store a synthetic result. The same project, request ID and exact result return the existing receipt.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn record_result(
        &self,
        scope: Scope,
        Parameters(input): Parameters<Write>,
    ) -> CallToolResult {
        if scope.role != Role::Collector || scope.project.as_ref() != input.project_id {
            return failure("FORBIDDEN");
        }
        if !valid_id(&input.request_id)
            || input.result.len() > 4096
            || !input.result.starts_with("[SYNTHETIC]")
        {
            return failure("INVALID_INPUT");
        }

        let store = self.store.clone();
        match tokio::task::spawn_blocking(move || {
            store.record(input.project_id, input.request_id, input.result)
        })
        .await
        {
            Ok(Ok(WriteOutcome::Stored(receipt))) => success("stored", receipt),
            Ok(Ok(WriteOutcome::Replayed(receipt))) => success("replayed", receipt),
            Ok(Ok(WriteOutcome::Conflict)) => failure("CONFLICT"),
            _ => failure("STORAGE_ERROR"),
        }
    }

    #[tool(
        description = "Retrieve a synthetic result and its original receipt by project and request ID.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_result(
        &self,
        scope: Scope,
        Parameters(input): Parameters<Lookup>,
    ) -> CallToolResult {
        if scope.project.as_ref() != input.project_id {
            return failure("FORBIDDEN");
        }
        if !valid_id(&input.request_id) {
            return failure("INVALID_INPUT");
        }

        let store = self.store.clone();
        match tokio::task::spawn_blocking(move || store.get(&input.project_id, &input.request_id))
            .await
        {
            Ok(Ok(Some(receipt))) => success("found", receipt),
            Ok(Ok(None)) => failure("NOT_FOUND"),
            _ => failure("STORAGE_ERROR"),
        }
    }
}

pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn success(status: &str, receipt: Receipt) -> CallToolResult {
    eprintln!(
        "{}",
        json!({
            "status": status,
            "project_id": receipt.project_id,
            "request_id": receipt.request_id,
            "receipt_id": receipt.receipt_id,
        })
    );
    CallToolResult::structured(json!({ "status": status, "receipt": receipt }))
}

fn failure(code: &str) -> CallToolResult {
    CallToolResult::structured_error(json!({ "code": code }))
}
