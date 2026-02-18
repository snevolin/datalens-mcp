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

type ToolJson = Json<Map<String, Value>>;

const DEFAULT_BASE_URL: &str = "https://api.datalens.tech";
const DEFAULT_API_VERSION: &str = "0";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const METHOD_CATALOG_SNAPSHOT_DATE: &str = "2026-02-18";
const METHOD_CATALOG_SOURCE_URL: &str = "https://yandex.cloud/en/docs/datalens/openapi-ref/";

#[derive(Clone, Copy)]
struct MethodCatalogItem {
    method: &'static str,
    tool: &'static str,
    category: &'static str,
    experimental: bool,
}

const METHOD_CATALOG: &[MethodCatalogItem] = &[
    MethodCatalogItem {
        method: "getConnection",
        tool: "datalens_get_connection",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "createConnection",
        tool: "datalens_create_connection",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "updateConnection",
        tool: "datalens_update_connection",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "deleteConnection",
        tool: "datalens_delete_connection",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getDashboard",
        tool: "datalens_get_dashboard",
        category: "read",
        experimental: true,
    },
    MethodCatalogItem {
        method: "createDashboard",
        tool: "datalens_create_dashboard",
        category: "write",
        experimental: true,
    },
    MethodCatalogItem {
        method: "updateDashboard",
        tool: "datalens_update_dashboard",
        category: "write",
        experimental: true,
    },
    MethodCatalogItem {
        method: "deleteDashboard",
        tool: "datalens_delete_dashboard",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getDataset",
        tool: "datalens_get_dataset",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "createDataset",
        tool: "datalens_create_dataset",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "updateDataset",
        tool: "datalens_update_dataset",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "deleteDataset",
        tool: "datalens_delete_dataset",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "validateDataset",
        tool: "datalens_validate_dataset",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getEntriesRelations",
        tool: "datalens_get_entries_relations",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getEntries",
        tool: "datalens_get_entries",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getQLChart",
        tool: "datalens_get_ql_chart",
        category: "read",
        experimental: true,
    },
    MethodCatalogItem {
        method: "deleteQLChart",
        tool: "datalens_delete_ql_chart",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getWizardChart",
        tool: "datalens_get_wizard_chart",
        category: "read",
        experimental: true,
    },
    MethodCatalogItem {
        method: "deleteWizardChart",
        tool: "datalens_delete_wizard_chart",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getEditorChart",
        tool: "datalens_get_editor_chart",
        category: "read",
        experimental: true,
    },
    MethodCatalogItem {
        method: "deleteEditorChart",
        tool: "datalens_delete_editor_chart",
        category: "write",
        experimental: false,
    },
    MethodCatalogItem {
        method: "createEditorChart",
        tool: "datalens_create_editor_chart",
        category: "write",
        experimental: true,
    },
    MethodCatalogItem {
        method: "updateEditorChart",
        tool: "datalens_update_editor_chart",
        category: "write",
        experimental: true,
    },
    MethodCatalogItem {
        method: "getEntriesPermissions",
        tool: "datalens_get_entries_permissions",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "getAuditEntriesUpdates",
        tool: "datalens_get_audit_entries_updates",
        category: "read",
        experimental: false,
    },
    MethodCatalogItem {
        method: "listDirectory",
        tool: "datalens_list_directory",
        category: "read",
        experimental: false,
    },
];

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
struct GetChartArgs {
    #[serde(alias = "chartId")]
    chart_id: String,
    #[serde(default, alias = "workbookId")]
    workbook_id: Option<String>,
    #[serde(default, alias = "revId", alias = "rev_id")]
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetEntriesRelationsArgs {
    #[serde(alias = "entryIds")]
    entry_ids: Vec<String>,
    #[serde(default, alias = "linkDirection")]
    link_direction: Option<String>,
    #[serde(default, alias = "includePermissionsInfo")]
    include_permissions_info: Option<bool>,
    #[serde(default)]
    limit: Option<serde_json::Number>,
    #[serde(default, alias = "pageToken")]
    page_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DeleteChartArgs {
    #[serde(alias = "chartId")]
    chart_id: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateEditorChartArgs {
    entry: Value,
    #[serde(default)]
    mode: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateEditorChartArgs {
    entry: Value,
    mode: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EntriesPermissionsArgs {
    #[serde(alias = "entryIds")]
    entry_ids: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AuditEntriesUpdatesArgs {
    from: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    limit: Option<serde_json::Number>,
    #[serde(default, alias = "pageToken")]
    page_token: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct NoArgs {}

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
        self.call_rpc(&args.method, args.payload).await
    }

    #[tool(
        name = "datalens_list_methods",
        description = "List DataLens API methods known to this server, with MCP tool names and method categories."
    )]
    async fn datalens_list_methods(
        &self,
        Parameters(_args): Parameters<NoArgs>,
    ) -> Result<ToolJson, McpError> {
        let methods = METHOD_CATALOG
            .iter()
            .map(|item| {
                json!({
                    "method": item.method,
                    "mcpTool": item.tool,
                    "category": item.category,
                    "experimental": item.experimental,
                })
            })
            .collect::<Vec<_>>();

        let response = json!({
            "snapshotDate": METHOD_CATALOG_SNAPSHOT_DATE,
            "sourceUrl": METHOD_CATALOG_SOURCE_URL,
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
            payload.insert("createdBy".to_owned(), created_by);
        }
        if let Some(order_by) = args.order_by {
            payload.insert("orderBy".to_owned(), order_by);
        }
        if let Some(filters) = args.filters {
            payload.insert("filters".to_owned(), filters);
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
            payload.insert("filters".to_owned(), filters);
        }
        if let Some(order_by) = args.order_by {
            payload.insert("orderBy".to_owned(), order_by);
        }
        if let Some(created_by) = args.created_by {
            payload.insert("createdBy".to_owned(), created_by);
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
            payload.insert("ids".to_owned(), ids);
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
        payload.insert("data".to_owned(), args.data);
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
        payload.insert("entry".to_owned(), args.entry);
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
        payload.insert("entry".to_owned(), args.entry);
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
        payload.insert("dataset".to_owned(), args.dataset);
        if let Some(created_via) = args.created_via {
            payload.insert("created_via".to_owned(), created_via);
        }
        if let Some(dir_path) = args.dir_path {
            payload.insert("dir_path".to_owned(), Value::String(dir_path));
        }
        if let Some(name) = args.name {
            payload.insert("name".to_owned(), Value::String(name));
        }
        if let Some(options) = args.options {
            payload.insert("options".to_owned(), options);
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
        if let Some(data) = args.data {
            payload.insert("data".to_owned(), data);
        }
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
        if let Some(data) = args.data {
            payload.insert("data".to_owned(), data);
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("validateDataset", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_get_entries_relations",
        description = "Call getEntriesRelations. Required: entry_ids."
    )]
    async fn datalens_get_entries_relations(
        &self,
        Parameters(args): Parameters<GetEntriesRelationsArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert(
            "entryIds".to_owned(),
            Value::Array(args.entry_ids.into_iter().map(Value::String).collect()),
        );
        if let Some(link_direction) = args.link_direction {
            payload.insert("linkDirection".to_owned(), Value::String(link_direction));
        }
        if let Some(include_permissions_info) = args.include_permissions_info {
            payload.insert(
                "includePermissionsInfo".to_owned(),
                Value::Bool(include_permissions_info),
            );
        }
        if let Some(limit) = args.limit {
            payload.insert("limit".to_owned(), Value::Number(limit));
        }
        if let Some(page_token) = args.page_token {
            payload.insert("pageToken".to_owned(), Value::String(page_token));
        }
        if let Some(scope) = args.scope {
            payload.insert("scope".to_owned(), Value::String(scope));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getEntriesRelations", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_get_ql_chart",
        description = "Call getQLChart by chart_id."
    )]
    async fn datalens_get_ql_chart(
        &self,
        Parameters(args): Parameters<GetChartArgs>,
    ) -> Result<ToolJson, McpError> {
        self.call_rpc("getQLChart", Value::Object(build_get_chart_payload(args)))
            .await
    }

    #[tool(
        name = "datalens_delete_ql_chart",
        description = "Call deleteQLChart by chart_id."
    )]
    async fn datalens_delete_ql_chart(
        &self,
        Parameters(args): Parameters<DeleteChartArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("chartId".to_owned(), Value::String(args.chart_id));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteQLChart", Value::Object(payload)).await
    }

    #[tool(
        name = "datalens_get_wizard_chart",
        description = "Call getWizardChart by chart_id."
    )]
    async fn datalens_get_wizard_chart(
        &self,
        Parameters(args): Parameters<GetChartArgs>,
    ) -> Result<ToolJson, McpError> {
        self.call_rpc(
            "getWizardChart",
            Value::Object(build_get_chart_payload(args)),
        )
        .await
    }

    #[tool(
        name = "datalens_delete_wizard_chart",
        description = "Call deleteWizardChart by chart_id."
    )]
    async fn datalens_delete_wizard_chart(
        &self,
        Parameters(args): Parameters<DeleteChartArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("chartId".to_owned(), Value::String(args.chart_id));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteWizardChart", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_get_editor_chart",
        description = "Call getEditorChart by chart_id."
    )]
    async fn datalens_get_editor_chart(
        &self,
        Parameters(args): Parameters<GetChartArgs>,
    ) -> Result<ToolJson, McpError> {
        self.call_rpc(
            "getEditorChart",
            Value::Object(build_get_chart_payload(args)),
        )
        .await
    }

    #[tool(
        name = "datalens_delete_editor_chart",
        description = "Call deleteEditorChart by chart_id."
    )]
    async fn datalens_delete_editor_chart(
        &self,
        Parameters(args): Parameters<DeleteChartArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("chartId".to_owned(), Value::String(args.chart_id));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("deleteEditorChart", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_create_editor_chart",
        description = "Call createEditorChart. Required: entry. Optional: mode (`save` or `publish`)."
    )]
    async fn datalens_create_editor_chart(
        &self,
        Parameters(args): Parameters<CreateEditorChartArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("entry".to_owned(), args.entry);
        if let Some(mode) = args.mode {
            payload.insert("mode".to_owned(), Value::String(mode));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("createEditorChart", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_update_editor_chart",
        description = "Call updateEditorChart. Required: entry, mode (`save` or `publish`)."
    )]
    async fn datalens_update_editor_chart(
        &self,
        Parameters(args): Parameters<UpdateEditorChartArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("entry".to_owned(), args.entry);
        payload.insert("mode".to_owned(), Value::String(args.mode));
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("updateEditorChart", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_get_entries_permissions",
        description = "Call getEntriesPermissions. Required: entry_ids."
    )]
    async fn datalens_get_entries_permissions(
        &self,
        Parameters(args): Parameters<EntriesPermissionsArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert(
            "entryIds".to_owned(),
            Value::Array(args.entry_ids.into_iter().map(Value::String).collect()),
        );
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getEntriesPermissions", Value::Object(payload))
            .await
    }

    #[tool(
        name = "datalens_get_audit_entries_updates",
        description = "Call getAuditEntriesUpdates. Required: from."
    )]
    async fn datalens_get_audit_entries_updates(
        &self,
        Parameters(args): Parameters<AuditEntriesUpdatesArgs>,
    ) -> Result<ToolJson, McpError> {
        let mut payload = Map::new();
        payload.insert("from".to_owned(), Value::String(args.from));
        if let Some(to) = args.to {
            payload.insert("to".to_owned(), Value::String(to));
        }
        if let Some(limit) = args.limit {
            payload.insert("limit".to_owned(), Value::Number(limit));
        }
        if let Some(page_token) = args.page_token {
            payload.insert("pageToken".to_owned(), Value::String(page_token));
        }
        extend_with_extra(&mut payload, args.extra);

        self.call_rpc("getAuditEntriesUpdates", Value::Object(payload))
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DataLensServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Yandex DataLens MCP server. Configure DATALENS_ORG_ID and YC_IAM_TOKEN (or DATALENS_IAM_TOKEN) before calling tools. Use datalens_list_methods to discover typed tools and API methods."
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

fn build_get_chart_payload(args: GetChartArgs) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("chartId".to_owned(), Value::String(args.chart_id));
    if let Some(workbook_id) = args.workbook_id {
        payload.insert("workbookId".to_owned(), Value::String(workbook_id));
    }
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
    payload
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
}
