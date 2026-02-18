use std::{collections::BTreeMap, env, sync::OnceLock, time::Duration};

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

type ToolJson = Json<Map<String, Value>>;

const DEFAULT_BASE_URL: &str = "https://api.datalens.tech";
const DEFAULT_API_VERSION: &str = "0";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MethodRegistry {
    snapshot_date: String,
    source_url: String,
    openapi_version: Option<String>,
    api_info: Value,
    methods: Vec<MethodRegistryItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MethodRegistryItem {
    method: String,
    category: String,
    experimental: bool,
    typed_tool: Option<String>,
    invoke_with: String,
    summary: Option<String>,
    description: Option<String>,
    request_schema: Value,
}

static METHOD_REGISTRY: OnceLock<MethodRegistry> = OnceLock::new();

fn method_registry() -> &'static MethodRegistry {
    METHOD_REGISTRY.get_or_init(|| {
        serde_json::from_str(include_str!("../openapi/datalens-rpc-methods.json"))
            .expect("embedded DataLens method registry must be valid JSON")
    })
}

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
    #[serde(default, alias = "createdBy")]
    created_by: Option<Value>,
    #[serde(default, alias = "orderBy")]
    order_by: Option<Value>,
    #[serde(default)]
    filters: Option<Value>,
    #[serde(default)]
    page: Option<serde_json::Number>,
    #[serde(default, alias = "pageSize")]
    page_size: Option<serde_json::Number>,
    #[serde(default, alias = "includePermissionsInfo")]
    include_permissions_info: Option<bool>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetDatasetArgs {
    #[serde(alias = "datasetId")]
    dataset_id: String,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(default, alias = "revId", alias = "rev_id")]
    rev_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetConnectionArgs {
    #[serde(alias = "connectionId")]
    connection_id: String,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(default, alias = "bindedDatasetId")]
    binded_dataset_id: Option<String>,
    #[serde(default, alias = "revId", alias = "rev_id")]
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
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetEntriesArgs {
    #[serde(default, alias = "excludeLocked")]
    exclude_locked: Option<bool>,
    #[serde(default, alias = "includeData")]
    include_data: Option<bool>,
    #[serde(default, alias = "includeLinks")]
    include_links: Option<bool>,
    #[serde(default)]
    filters: Option<Value>,
    #[serde(default, alias = "orderBy")]
    order_by: Option<Value>,
    #[serde(default, alias = "createdBy")]
    created_by: Option<Value>,
    #[serde(default)]
    page: Option<serde_json::Number>,
    #[serde(default, alias = "pageSize")]
    page_size: Option<serde_json::Number>,
    #[serde(default, alias = "includePermissionsInfo")]
    include_permissions_info: Option<bool>,
    #[serde(default, alias = "ignoreWorkbookEntries")]
    ignore_workbook_entries: Option<bool>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    ids: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateConnectionArgs {
    #[serde(rename = "type")]
    connection_type: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateConnectionArgs {
    #[serde(alias = "connectionId")]
    connection_id: String,
    data: Value,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DeleteConnectionArgs {
    #[serde(alias = "connectionId")]
    connection_id: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateDashboardArgs {
    entry: Value,
    mode: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateDashboardArgs {
    entry: Value,
    mode: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DeleteDashboardArgs {
    #[serde(alias = "dashboardId")]
    dashboard_id: String,
    #[serde(default, alias = "lockToken")]
    lock_token: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateDatasetArgs {
    dataset: Value,
    #[serde(default, alias = "createdVia")]
    created_via: Option<Value>,
    #[serde(default, alias = "dirPath")]
    dir_path: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    options: Option<Value>,
    #[serde(default)]
    preview: Option<bool>,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateDatasetArgs {
    #[serde(alias = "datasetId")]
    dataset_id: String,
    #[serde(default)]
    data: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DeleteDatasetArgs {
    #[serde(alias = "datasetId")]
    dataset_id: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ValidateDatasetArgs {
    #[serde(alias = "datasetId")]
    dataset_id: String,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(default)]
    data: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct NoArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetMethodSchemaArgs {
    #[serde(alias = "methodName")]
    method: String,
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
    ) -> Result<ToolJson, McpError> {
        let payload = normalize_json_value(args.payload, "payload")?;
        self.call_rpc(&args.method, payload).await
    }

    #[tool(
        name = "datalens_list_methods",
        description = "List DataLens API methods known to this server, with MCP tool names and method categories."
    )]
    async fn datalens_list_methods(
        &self,
        Parameters(_args): Parameters<NoArgs>,
    ) -> Result<ToolJson, McpError> {
        let registry = method_registry();
        let methods = registry
            .methods
            .iter()
            .map(|item| {
                let mcp_tool = item
                    .typed_tool
                    .clone()
                    .unwrap_or_else(|| "datalens_rpc".to_owned());
                json!({
                    "method": item.method,
                    "mcpTool": mcp_tool,
                    "typedTool": item.typed_tool,
                    "invokeWith": item.invoke_with,
                    "category": item.category,
                    "experimental": item.experimental,
                    "summary": item.summary,
                })
            })
            .collect::<Vec<_>>();

        let response = json!({
            "snapshotDate": registry.snapshot_date,
            "sourceUrl": registry.source_url,
            "openapiVersion": registry.openapi_version,
            "apiInfo": registry.api_info,
            "totalMethods": methods.len(),
            "genericTool": "datalens_rpc",
            "methods": methods,
        });
        let response = response.as_object().cloned().ok_or_else(|| {
            McpError::internal_error("failed to build method catalog response object", None)
        })?;

        Ok(Json(response))
    }

    #[tool(
        name = "datalens_get_method_schema",
        description = "Return OpenAPI request schema and invocation hints for a DataLens RPC method."
    )]
    async fn datalens_get_method_schema(
        &self,
        Parameters(args): Parameters<GetMethodSchemaArgs>,
    ) -> Result<ToolJson, McpError> {
        let registry = method_registry();
        let method = registry
            .methods
            .iter()
            .find(|item| item.method.eq_ignore_ascii_case(&args.method))
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("Unknown DataLens RPC method: {}", args.method),
                    Some(json!({
                        "hint": "Call datalens_list_methods first to discover valid methods."
                    })),
                )
            })?;

        let response = json!({
            "snapshotDate": registry.snapshot_date,
            "sourceUrl": registry.source_url,
            "openapiVersion": registry.openapi_version,
            "method": method.method,
            "category": method.category,
            "experimental": method.experimental,
            "typedTool": method.typed_tool,
            "invokeWith": method.invoke_with,
            "summary": method.summary,
            "description": method.description,
            "requestSchema": method.request_schema,
        });
        let response = response.as_object().cloned().ok_or_else(|| {
            McpError::internal_error("failed to build method schema response object", None)
        })?;

        Ok(Json(response))
    }

    #[tool(
        name = "datalens_list_directory",
        description = "Call listDirectory. By default, lists the root path '/'."
    )]
    async fn datalens_list_directory(
        &self,
        Parameters(args): Parameters<ListDirectoryArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("path".to_owned(), Value::String(args.path));
        if let Some(created_by) = args.created_by {
            payload.insert(
                "createdBy".to_owned(),
                normalize_json_value(created_by, "createdBy")?,
            );
        }
        if let Some(order_by) = args.order_by {
            payload.insert(
                "orderBy".to_owned(),
                normalize_json_value(order_by, "orderBy")?,
            );
        }
        if let Some(filters) = args.filters {
            payload.insert(
                "filters".to_owned(),
                normalize_json_value(filters, "filters")?,
            );
        }
        if let Some(page) = args.page {
            payload.insert("page".to_owned(), Value::Number(page));
        }
        if let Some(page_size) = args.page_size {
            payload.insert("pageSize".to_owned(), Value::Number(page_size));
        }
        if let Some(include_permissions_info) = args.include_permissions_info {
            payload.insert(
                "includePermissionsInfo".to_owned(),
                Value::Bool(include_permissions_info),
            );
        }
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
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        if let Some(exclude_locked) = args.exclude_locked {
            payload.insert("excludeLocked".to_owned(), Value::Bool(exclude_locked));
        }
        if let Some(include_data) = args.include_data {
            payload.insert("includeData".to_owned(), Value::Bool(include_data));
        }
        if let Some(include_links) = args.include_links {
            payload.insert("includeLinks".to_owned(), Value::Bool(include_links));
        }
        if let Some(filters) = args.filters {
            payload.insert(
                "filters".to_owned(),
                normalize_json_value(filters, "filters")?,
            );
        }
        if let Some(order_by) = args.order_by {
            payload.insert(
                "orderBy".to_owned(),
                normalize_json_value(order_by, "orderBy")?,
            );
        }
        if let Some(created_by) = args.created_by {
            payload.insert(
                "createdBy".to_owned(),
                normalize_json_value(created_by, "createdBy")?,
            );
        }
        if let Some(page) = args.page {
            payload.insert("page".to_owned(), Value::Number(page));
        }
        if let Some(page_size) = args.page_size {
            payload.insert("pageSize".to_owned(), Value::Number(page_size));
        }
        if let Some(include_permissions_info) = args.include_permissions_info {
            payload.insert(
                "includePermissionsInfo".to_owned(),
                Value::Bool(include_permissions_info),
            );
        }
        if let Some(ignore_workbook_entries) = args.ignore_workbook_entries {
            payload.insert(
                "ignoreWorkbookEntries".to_owned(),
                Value::Bool(ignore_workbook_entries),
            );
        }
        if let Some(scope) = args.scope {
            payload.insert("scope".to_owned(), Value::String(scope));
        }
        if let Some(ids) = args.ids {
            payload.insert("ids".to_owned(), normalize_json_value(ids, "ids")?);
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getEntries", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_get_dataset",
        description = "Call getDataset by dataset_id. Optional: workbook_id, rev_id and other request fields."
    )]
    async fn datalens_get_dataset(
        &self,
        Parameters(args): Parameters<GetDatasetArgs>,
    ) -> Result<ToolJson, McpError> {
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
    ) -> Result<ToolJson, McpError> {
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
        if let Some(workbook_id) = args.workbook_id {
            payload.insert("workbookId".to_owned(), Value::String(workbook_id));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getDashboard", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_get_connection",
        description = "Call getConnection by connection_id. Optional: workbook_id, binded_dataset_id, rev_id."
    )]
    async fn datalens_get_connection(
        &self,
        Parameters(args): Parameters<GetConnectionArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("connectionId".to_owned(), Value::String(args.connection_id));
        if let Some(workbook_id) = args.workbook_id {
            payload.insert("workbookId".to_owned(), Value::String(workbook_id));
        }
        if let Some(binded_dataset_id) = args.binded_dataset_id {
            payload.insert(
                "bindedDatasetId".to_owned(),
                Value::String(binded_dataset_id),
            );
        }
        if let Some(rev_id) = args.rev_id {
            payload.insert("rev_id".to_owned(), Value::String(rev_id));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getConnection", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_create_connection",
        description = "Call createConnection. Include required connection fields for the selected `type`."
    )]
    async fn datalens_create_connection(
        &self,
        Parameters(args): Parameters<CreateConnectionArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("type".to_owned(), Value::String(args.connection_type));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("createConnection", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_update_connection",
        description = "Call updateConnection. Required: connection_id, data."
    )]
    async fn datalens_update_connection(
        &self,
        Parameters(args): Parameters<UpdateConnectionArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("connectionId".to_owned(), Value::String(args.connection_id));
        payload.insert("data".to_owned(), normalize_json_value(args.data, "data")?);
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("updateConnection", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_delete_connection",
        description = "Call deleteConnection by connection_id."
    )]
    async fn datalens_delete_connection(
        &self,
        Parameters(args): Parameters<DeleteConnectionArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("connectionId".to_owned(), Value::String(args.connection_id));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteConnection", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_create_dashboard",
        description = "Call createDashboard. Required: entry, mode (`save` or `publish`)."
    )]
    async fn datalens_create_dashboard(
        &self,
        Parameters(args): Parameters<CreateDashboardArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert(
            "entry".to_owned(),
            normalize_json_value(args.entry, "entry")?,
        );
        payload.insert("mode".to_owned(), Value::String(args.mode));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("createDashboard", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_update_dashboard",
        description = "Call updateDashboard. Required: entry, mode (`save` or `publish`)."
    )]
    async fn datalens_update_dashboard(
        &self,
        Parameters(args): Parameters<UpdateDashboardArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert(
            "entry".to_owned(),
            normalize_json_value(args.entry, "entry")?,
        );
        payload.insert("mode".to_owned(), Value::String(args.mode));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("updateDashboard", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_delete_dashboard",
        description = "Call deleteDashboard by dashboard_id. Optional: lock_token."
    )]
    async fn datalens_delete_dashboard(
        &self,
        Parameters(args): Parameters<DeleteDashboardArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("dashboardId".to_owned(), Value::String(args.dashboard_id));
        if let Some(lock_token) = args.lock_token {
            payload.insert("lockToken".to_owned(), Value::String(lock_token));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteDashboard", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_create_dataset",
        description = "Call createDataset. Required: dataset. For workbook-scoped creation, pass workbook_id."
    )]
    async fn datalens_create_dataset(
        &self,
        Parameters(args): Parameters<CreateDatasetArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert(
            "dataset".to_owned(),
            normalize_json_value(args.dataset, "dataset")?,
        );
        if let Some(created_via) = args.created_via {
            payload.insert(
                "created_via".to_owned(),
                normalize_json_value(created_via, "created_via")?,
            );
        }
        if let Some(dir_path) = args.dir_path {
            payload.insert("dir_path".to_owned(), Value::String(dir_path));
        }
        if let Some(name) = args.name {
            payload.insert("name".to_owned(), Value::String(name));
        }
        if let Some(options) = args.options {
            payload.insert(
                "options".to_owned(),
                normalize_json_value(options, "options")?,
            );
        }
        if let Some(preview) = args.preview {
            payload.insert("preview".to_owned(), Value::Bool(preview));
        }
        if let Some(workbook_id) = args.workbook_id {
            payload.insert("workbook_id".to_owned(), Value::String(workbook_id));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("createDataset", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_update_dataset",
        description = "Call updateDataset by dataset_id. Optional: data."
    )]
    async fn datalens_update_dataset(
        &self,
        Parameters(args): Parameters<UpdateDatasetArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("datasetId".to_owned(), Value::String(args.dataset_id));
        let data = args.data.unwrap_or_else(|| Value::Object(Map::new()));
        payload.insert("data".to_owned(), normalize_json_value(data, "data")?);
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("updateDataset", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_delete_dataset",
        description = "Call deleteDataset by dataset_id."
    )]
    async fn datalens_delete_dataset(
        &self,
        Parameters(args): Parameters<DeleteDatasetArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("datasetId".to_owned(), Value::String(args.dataset_id));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteDataset", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_validate_dataset",
        description = "Call validateDataset by dataset_id. Optional: workbook_id, data."
    )]
    async fn datalens_validate_dataset(
        &self,
        Parameters(args): Parameters<ValidateDatasetArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("datasetId".to_owned(), Value::String(args.dataset_id));
        if let Some(workbook_id) = args.workbook_id {
            payload.insert("workbookId".to_owned(), Value::String(workbook_id));
        }
        let data = args.data.unwrap_or_else(|| Value::Object(Map::new()));
        payload.insert("data".to_owned(), normalize_json_value(data, "data")?);
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("validateDataset", Value::Object(payload))
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DataLensServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Yandex DataLens MCP server. Configure DATALENS_ORG_ID and YC_IAM_TOKEN (or DATALENS_IAM_TOKEN) before calling tools. For broad RPC usage: call datalens_list_methods, then datalens_get_method_schema for the chosen method, then call either a typed tool or datalens_rpc."
                    .to_owned(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl DataLensServer {
    async fn call_rpc(&self, method: &str, payload: Value) -> Result<ToolJson, McpError> {
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
            return Ok(Json(Map::new()));
        }

        let parsed = serde_json::from_str::<Map<String, Value>>(&body).map_err(|error| {
            McpError::internal_error(
                format!("DataLens API returned invalid or non-object JSON: {error}"),
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

fn normalize_json_value(value: Value, field_name: &str) -> Result<Value, McpError> {
    let Value::String(raw) = value else {
        return Ok(value);
    };

    let trimmed = raw.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Ok(Value::String(raw));
    }

    serde_json::from_str::<Value>(trimmed).map_err(|error| {
        McpError::invalid_params(
            format!("field `{field_name}` must be valid JSON when passed as a string: {error}"),
            None,
        )
    })
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

fn error_chain_contains(error: &(dyn std::error::Error + 'static), needle: &str) -> bool {
    let mut current = Some(error);
    while let Some(err) = current {
        if err.to_string().contains(needle) {
            return true;
        }
        current = err.source();
    }
    false
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
    let service = server.serve(stdio()).await.map_err(|error| {
        if error_chain_contains(&error, "connection closed: initialized request")
            || error_chain_contains(&error, "initialized request")
        {
            anyhow::anyhow!(
                "MCP client is not connected: this binary is a stdio MCP server and must be launched by an MCP host (Codex/Cursor/Claude), not directly from a shell."
            )
        } else {
            anyhow::Error::new(error).context("failed to start MCP stdio service")
        }
    })?;

    service
        .waiting()
        .await
        .context("MCP service terminated unexpectedly")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method, path},
    };

    fn test_config(base_url: String) -> AppConfig {
        AppConfig {
            base_url,
            api_version: "0".to_owned(),
            org_id: Some("org-123".to_owned()),
            subject_token: Some("token-abc".to_owned()),
            timeout: Duration::from_secs(5),
        }
    }

    fn test_server(base_url: String) -> DataLensServer {
        let cfg = test_config(base_url);
        let http = Client::builder()
            .timeout(cfg.timeout)
            .build()
            .expect("test HTTP client must initialize");

        DataLensServer {
            tool_router: ToolRouter::new(),
            http,
            cfg,
        }
    }

    #[test]
    fn parse_response_data_returns_json_when_valid() {
        let value = parse_response_data(r#"{"ok":true,"n":1}"#);
        assert_eq!(value, json!({"ok": true, "n": 1}));
    }

    #[test]
    fn truncate_utf8_keeps_char_boundaries() {
        let input = "abc🙂🙂";
        let out = truncate_utf8(input, 5);
        assert!(out.starts_with("abc"));
        assert!(!out.contains('\u{fffd}'));
    }

    #[test]
    fn normalize_json_value_parses_stringified_object() {
        let value = normalize_json_value(Value::String(r#"{"a":1}"#.to_owned()), "payload")
            .expect("must parse stringified JSON object");
        assert_eq!(value, json!({"a": 1}));
    }

    #[test]
    fn normalize_json_value_keeps_plain_string() {
        let value = normalize_json_value(Value::String("plain-text".to_owned()), "payload")
            .expect("plain string must be preserved");
        assert_eq!(value, Value::String("plain-text".to_owned()));
    }

    #[test]
    fn get_dashboard_args_accept_legacy_include_permissions_info_alias() {
        let args: GetDashboardArgs = serde_json::from_value(json!({
            "dashboardId": "dash-1",
            "includePermissionsInfo": true
        }))
        .expect("deserialization must succeed");

        assert_eq!(args.include_permissions, Some(true));
    }

    #[tokio::test]
    async fn datalens_list_methods_includes_write_methods() {
        let server = test_server("http://127.0.0.1".to_owned());

        let response = server
            .datalens_list_methods(Parameters(NoArgs::default()))
            .await
            .expect("list methods must succeed");

        let methods = response
            .0
            .get("methods")
            .and_then(Value::as_array)
            .expect("methods must be an array");

        assert!(
            methods.iter().any(|method| {
                method.get("method") == Some(&Value::String("createDataset".to_owned()))
                    && method.get("mcpTool")
                        == Some(&Value::String("datalens_create_dataset".to_owned()))
                    && method.get("category") == Some(&Value::String("write".to_owned()))
            }),
            "method catalog must include createDataset write wrapper"
        );
    }

    #[tokio::test]
    async fn datalens_list_methods_includes_full_openapi_snapshot() {
        let server = test_server("http://127.0.0.1".to_owned());

        let response = server
            .datalens_list_methods(Parameters(NoArgs::default()))
            .await
            .expect("list methods must succeed");

        let total_methods = response
            .0
            .get("totalMethods")
            .and_then(Value::as_u64)
            .expect("totalMethods must be present and numeric");
        assert_eq!(
            total_methods, 60,
            "embedded method snapshot should expose all RPC methods"
        );
    }

    #[tokio::test]
    async fn datalens_get_method_schema_returns_invoke_hints() {
        let server = test_server("http://127.0.0.1".to_owned());

        let response = server
            .datalens_get_method_schema(Parameters(GetMethodSchemaArgs {
                method: "createDataset".to_owned(),
            }))
            .await
            .expect("get method schema must succeed");

        assert_eq!(
            response.0.get("method"),
            Some(&Value::String("createDataset".to_owned()))
        );
        assert_eq!(
            response.0.get("typedTool"),
            Some(&Value::String("datalens_create_dataset".to_owned()))
        );
        assert_eq!(
            response.0.get("invokeWith"),
            Some(&Value::String("datalens_create_dataset".to_owned()))
        );
        assert!(response.0.get("requestSchema").is_some());
    }

    #[tokio::test]
    async fn call_rpc_validates_payload_object() {
        let server = test_server("http://127.0.0.1".to_owned());

        let err = match server
            .call_rpc("listDirectory", json!(["not-an-object"]))
            .await
        {
            Ok(_) => panic!("must reject non-object payload"),
            Err(err) => err,
        };

        assert_eq!(err.message, "payload must be a JSON object");
    }

    #[tokio::test]
    async fn call_rpc_sends_expected_request_shape() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/listDirectory"))
            .and(header("x-dl-api-version", "0"))
            .and(header("x-dl-org-id", "org-123"))
            .and(header("x-yacloud-subjecttoken", "token-abc"))
            .and(header("x-dl-auth-token", "OAuth token-abc"))
            .and(body_json(json!({"path": "/"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"entries": []})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());

        let response = server
            .call_rpc("listDirectory", json!({"path": "/"}))
            .await
            .expect("request must succeed");

        assert_eq!(Value::Object(response.0), json!({"entries": []}));
    }

    #[tokio::test]
    async fn datalens_get_dataset_uses_rev_id_as_rev_id_field() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/getDataset"))
            .and(body_json(json!({
                "datasetId": "ds-1",
                "workbookId": "wb-1",
                "rev_id": "r-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());

        let result = server
            .datalens_get_dataset(Parameters(GetDatasetArgs {
                dataset_id: "ds-1".to_owned(),
                workbook_id: Some("wb-1".to_owned()),
                rev_id: Some("r-1".to_owned()),
                extra: BTreeMap::new(),
            }))
            .await
            .expect("tool call must succeed");

        assert_eq!(Value::Object(result.0), json!({"ok": true}));
    }

    #[tokio::test]
    async fn datalens_create_dataset_calls_create_dataset_rpc_method() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/createDataset"))
            .and(body_json(json!({
                "dataset": {},
                "name": "my-dataset",
                "workbook_id": "wb-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"datasetId": "ds-1"})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_create_dataset(Parameters(CreateDatasetArgs {
                dataset: json!({}),
                created_via: None,
                dir_path: None,
                name: Some("my-dataset".to_owned()),
                options: None,
                preview: None,
                workbook_id: Some("wb-1".to_owned()),
                extra: BTreeMap::new(),
            }))
            .await
            .expect("tool call must succeed");

        assert_eq!(Value::Object(result.0), json!({"datasetId": "ds-1"}));
    }

    #[tokio::test]
    async fn datalens_rpc_parses_stringified_json_payload() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/listDirectory"))
            .and(body_json(json!({ "path": "/" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"entries": []})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_rpc(Parameters(DatalensRpcArgs {
                method: "listDirectory".to_owned(),
                payload: Value::String(r#"{"path":"/"}"#.to_owned()),
            }))
            .await
            .expect("stringified payload must be parsed and sent as JSON object");

        assert_eq!(Value::Object(result.0), json!({"entries": []}));
    }

    #[tokio::test]
    async fn datalens_create_dashboard_parses_stringified_entry() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/createDashboard"))
            .and(body_json(json!({
                "entry": {"name": "dash"},
                "mode": "save"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"dashboardId": "d-1"})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_create_dashboard(Parameters(CreateDashboardArgs {
                entry: Value::String(r#"{"name":"dash"}"#.to_owned()),
                mode: "save".to_owned(),
                extra: BTreeMap::new(),
            }))
            .await
            .expect("stringified entry must be parsed to object");

        assert_eq!(Value::Object(result.0), json!({"dashboardId": "d-1"}));
    }

    #[tokio::test]
    async fn datalens_create_dataset_parses_stringified_dataset() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/createDataset"))
            .and(body_json(json!({
                "dataset": {},
                "workbook_id": "wb-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"datasetId": "ds-1"})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_create_dataset(Parameters(CreateDatasetArgs {
                dataset: Value::String("{}".to_owned()),
                created_via: None,
                dir_path: None,
                name: None,
                options: None,
                preview: None,
                workbook_id: Some("wb-1".to_owned()),
                extra: BTreeMap::new(),
            }))
            .await
            .expect("stringified dataset must be parsed to object");

        assert_eq!(Value::Object(result.0), json!({"datasetId": "ds-1"}));
    }

    #[tokio::test]
    async fn datalens_update_dataset_defaults_data_to_empty_object() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/updateDataset"))
            .and(body_json(json!({
                "datasetId": "ds-1",
                "data": {}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_update_dataset(Parameters(UpdateDatasetArgs {
                dataset_id: "ds-1".to_owned(),
                data: None,
                extra: BTreeMap::new(),
            }))
            .await
            .expect("missing data must default to {}");

        assert_eq!(Value::Object(result.0), json!({"ok": true}));
    }

    #[tokio::test]
    async fn datalens_validate_dataset_defaults_data_to_empty_object() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rpc/validateDataset"))
            .and(body_json(json!({
                "datasetId": "ds-1",
                "data": {}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let server = test_server(mock_server.uri());
        let result = server
            .datalens_validate_dataset(Parameters(ValidateDatasetArgs {
                dataset_id: "ds-1".to_owned(),
                workbook_id: None,
                data: None,
                extra: BTreeMap::new(),
            }))
            .await
            .expect("missing data must default to {}");

        assert_eq!(Value::Object(result.0), json!({"ok": true}));
    }
}
