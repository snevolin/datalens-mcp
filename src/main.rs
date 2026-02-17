use std::{collections::BTreeMap, env, time::Duration};

use anyhow::{Context, Result};
use reqwest::{
    Client,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

const DEFAULT_BASE_URL: &str = "https://api.datalens.tech";
const DEFAULT_API_VERSION: &str = "0";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone, Debug)]
struct AppConfig {
    base_url: String,
    api_version: String,
    org_id: Option<String>,
    subject_token: Option<String>,
    timeout: Duration,
}

impl AppConfig {
    fn from_env() -> Self {
        let timeout_seconds = parse_timeout_seconds();

        Self {
            base_url: env_non_empty("DATALENS_BASE_URL")
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned()),
            api_version: env_non_empty("DATALENS_API_VERSION")
                .unwrap_or_else(|| DEFAULT_API_VERSION.to_owned()),
            org_id: env_non_empty("DATALENS_ORG_ID"),
            subject_token: env_non_empty("DATALENS_IAM_TOKEN")
                .or_else(|| env_non_empty("YC_IAM_TOKEN"))
                .or_else(|| env_non_empty("DATALENS_SUBJECT_TOKEN")),
            timeout: Duration::from_secs(timeout_seconds),
        }
    }
}

#[derive(Clone)]
struct DataLensServer {
    tool_router: ToolRouter<Self>,
    http: Client,
    cfg: AppConfig,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DatalensRpcArgs {
    method: String,
    #[serde(default = "empty_json_object")]
    payload: Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListDirectoryArgs {
    #[serde(default = "default_root_path")]
    path: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct GetEntriesArgs {
    #[serde(flatten)]
    payload: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetDatasetArgs {
    #[serde(alias = "datasetId")]
    dataset_id: String,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(default, alias = "revId")]
    rev_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetDashboardArgs {
    #[serde(alias = "dashboardId")]
    dashboard_id: String,
    #[serde(default, alias = "revId")]
    rev_id: Option<String>,
    #[serde(
        default,
        alias = "includePermissions",
        alias = "includePermissionsInfo"
    )]
    include_permissions: Option<bool>,
    #[serde(default, alias = "includeLinks")]
    include_links: Option<bool>,
    #[serde(default, alias = "includeFavorite")]
    include_favorite: Option<bool>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[tool_router]
impl DataLensServer {
    fn new(cfg: AppConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(cfg.timeout)
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            tool_router: Self::tool_router(),
            http,
            cfg,
        })
    }

    #[tool(
        name = "datalens_rpc",
        description = "Call any DataLens RPC method by its method name and JSON payload."
    )]
    async fn datalens_rpc(
        &self,
        Parameters(args): Parameters<DatalensRpcArgs>,
    ) -> Result<Json<Value>, McpError> {
        self.call_rpc(&args.method, args.payload).await
    }

    #[tool(
        name = "datalens_list_directory",
        description = "Call listDirectory. By default, lists the root path '/'."
    )]
    async fn datalens_list_directory(
        &self,
        Parameters(args): Parameters<ListDirectoryArgs>,
    ) -> Result<Json<Value>, McpError> {
        let mut payload = Map::new();
        payload.insert("path".to_owned(), Value::String(args.path));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("listDirectory", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_get_entries",
        description = "Call getEntries. Pass any getEntries request fields."
    )]
    async fn datalens_get_entries(
        &self,
        Parameters(args): Parameters<GetEntriesArgs>,
    ) -> Result<Json<Value>, McpError> {
        let payload = Value::Object(args.payload.into_iter().collect());
        self.call_rpc("getEntries", payload).await
    }

    #[tool(
        name = "datalens_get_dataset",
        description = "Call getDataset by dataset_id. Optional: workbook_id, rev_id and other request fields."
    )]
    async fn datalens_get_dataset(
        &self,
        Parameters(args): Parameters<GetDatasetArgs>,
    ) -> Result<Json<Value>, McpError> {
        let mut payload = Map::new();
        payload.insert("datasetId".to_owned(), Value::String(args.dataset_id));

        if let Some(workbook_id) = args.workbook_id {
            payload.insert("workbookId".to_owned(), Value::String(workbook_id));
        }
        if let Some(rev_id) = args.rev_id {
            payload.insert("rev_id".to_owned(), Value::String(rev_id));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getDataset", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_get_dashboard",
        description = "Call getDashboard by dashboard_id. Optional: rev_id, include_permissions, include_links, include_favorite, branch and other fields."
    )]
    async fn datalens_get_dashboard(
        &self,
        Parameters(args): Parameters<GetDashboardArgs>,
    ) -> Result<Json<Value>, McpError> {
        let mut payload = Map::new();
        payload.insert("dashboardId".to_owned(), Value::String(args.dashboard_id));

        if let Some(rev_id) = args.rev_id {
            payload.insert("revId".to_owned(), Value::String(rev_id));
        }
        if let Some(include_permissions) = args.include_permissions {
            payload.insert(
                "includePermissions".to_owned(),
                Value::Bool(include_permissions),
            );
        }
        if let Some(include_links) = args.include_links {
            payload.insert("includeLinks".to_owned(), Value::Bool(include_links));
        }
        if let Some(include_favorite) = args.include_favorite {
            payload.insert("includeFavorite".to_owned(), Value::Bool(include_favorite));
        }
        if let Some(branch) = args.branch {
            payload.insert("branch".to_owned(), Value::String(branch));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getDashboard", Value::Object(payload)).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DataLensServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Yandex DataLens MCP server. Configure DATALENS_ORG_ID and YC_IAM_TOKEN (or DATALENS_IAM_TOKEN) before calling tools."
                    .to_owned(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl DataLensServer {
    async fn call_rpc(&self, method: &str, payload: Value) -> Result<Json<Value>, McpError> {
        if !payload.is_object() {
            return Err(McpError::invalid_params(
                "payload must be a JSON object",
                Some(json!({"method": method})),
            ));
        }

        let org_id = self.cfg.org_id.as_deref().ok_or_else(|| {
            McpError::invalid_request("DATALENS_ORG_ID environment variable is required", None)
        })?;
        let subject_token = self.cfg.subject_token.as_deref().ok_or_else(|| {
            McpError::invalid_request(
                "YC_IAM_TOKEN (or DATALENS_IAM_TOKEN) environment variable is required",
                None,
            )
        })?;

        let url = format!("{}/rpc/{}", self.cfg.base_url.trim_end_matches('/'), method);
        debug!(method = %method, url = %url, "calling DataLens API");

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        add_header(&mut headers, "x-dl-api-version", &self.cfg.api_version)?;
        add_header(&mut headers, "x-dl-org-id", org_id)?;
        add_header(&mut headers, "x-yacloud-subjecttoken", subject_token)?;

        let legacy_auth_header = if subject_token.starts_with("OAuth ") {
            subject_token.to_owned()
        } else {
            format!("OAuth {subject_token}")
        };
        add_header(&mut headers, "x-dl-auth-token", &legacy_auth_header)?;

        let response = self
            .http
            .post(url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                McpError::internal_error(
                    format!("failed to reach DataLens API: {error}"),
                    Some(json!({"method": method})),
                )
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|error| {
            McpError::internal_error(format!("failed to read response: {error}"), None)
        })?;

        if !status.is_success() {
            let response_data = parse_response_data(&body);
            return Err(McpError::internal_error(
                format!("DataLens API returned {status} for method {method}"),
                Some(json!({
                    "method": method,
                    "status": status.as_u16(),
                    "response": response_data,
                })),
            ));
        }

        if body.trim().is_empty() {
            return Ok(Json(Value::Object(Map::new())));
        }

        let parsed = serde_json::from_str::<Value>(&body).map_err(|error| {
            McpError::internal_error(
                format!("DataLens API returned invalid JSON: {error}"),
                Some(json!({
                    "method": method,
                    "body": truncate_utf8(&body, 2000),
                })),
            )
        })?;

        Ok(Json(parsed))
    }
}

fn extend_with_extra(target: &mut Map<String, Value>, extra: BTreeMap<String, Value>) {
    for (key, value) in extra {
        target.insert(key, value);
    }
}

fn add_header(headers: &mut HeaderMap, key: &str, value: &str) -> Result<(), McpError> {
    let name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
        McpError::invalid_params(format!("invalid header name `{key}`: {error}"), None)
    })?;
    let value = HeaderValue::from_str(value).map_err(|error| {
        McpError::invalid_params(format!("invalid header value for `{key}`: {error}"), None)
    })?;

    headers.insert(name, value);
    Ok(())
}

fn parse_response_data(body: &str) -> Value {
    match serde_json::from_str::<Value>(body) {
        Ok(json) => json,
        Err(_) => Value::String(truncate_utf8(body, 2000)),
    }
}

fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }

    let mut end = max_bytes;
    while !input.is_char_boundary(end) {
        end -= 1;
    }

    format!("{}...(truncated)", &input[..end])
}

fn env_non_empty(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_timeout_seconds() -> u64 {
    match env_non_empty("DATALENS_TIMEOUT_SECONDS") {
        Some(raw) => match raw.parse::<u64>() {
            Ok(value) if value > 0 => value,
            Ok(_) => {
                warn!(
                    "DATALENS_TIMEOUT_SECONDS must be a positive integer, using default {DEFAULT_TIMEOUT_SECONDS}"
                );
                DEFAULT_TIMEOUT_SECONDS
            }
            Err(error) => {
                warn!(
                    "Failed to parse DATALENS_TIMEOUT_SECONDS='{raw}': {error}; using default {DEFAULT_TIMEOUT_SECONDS}"
                );
                DEFAULT_TIMEOUT_SECONDS
            }
        },
        None => DEFAULT_TIMEOUT_SECONDS,
    }
}

fn default_root_path() -> String {
    "/".to_owned()
}

fn empty_json_object() -> Value {
    Value::Object(Map::new())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cfg = AppConfig::from_env();
    info!(
        base_url = %cfg.base_url,
        api_version = %cfg.api_version,
        "starting datalens-mcp server"
    );

    if cfg.org_id.is_none() {
        warn!("DATALENS_ORG_ID is not set; tool calls will fail until it is configured");
    }
    if cfg.subject_token.is_none() {
        warn!(
            "YC_IAM_TOKEN / DATALENS_IAM_TOKEN is not set; tool calls will fail until it is configured"
        );
    }

    let server = DataLensServer::new(cfg).context("failed to initialize server")?;
    let service = server
        .serve(stdio())
        .await
        .context("failed to start MCP stdio service")?;

    service
        .waiting()
        .await
        .context("MCP service terminated unexpectedly")?;

    Ok(())
}
