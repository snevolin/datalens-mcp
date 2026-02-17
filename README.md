# datalens-mcp

Rust MCP server for the Yandex DataLens Public API (`https://api.datalens.tech`).

This server uses the MCP `stdio` transport and exposes DataLens RPC methods as MCP tools.

## Supported Tools

- Utility:
  - `datalens_list_methods`: returns known DataLens API methods, mapped MCP tool names, categories, and snapshot metadata.
  - `datalens_rpc`: generic fallback for any method under `/rpc/{method}`.
- Typed wrappers (method-specific):
  - `datalens_get_connection` -> `getConnection`
  - `datalens_create_connection` -> `createConnection`
  - `datalens_update_connection` -> `updateConnection`
  - `datalens_delete_connection` -> `deleteConnection`
  - `datalens_get_dashboard` -> `getDashboard`
  - `datalens_create_dashboard` -> `createDashboard`
  - `datalens_update_dashboard` -> `updateDashboard`
  - `datalens_delete_dashboard` -> `deleteDashboard`
  - `datalens_get_dataset` -> `getDataset`
  - `datalens_create_dataset` -> `createDataset`
  - `datalens_update_dataset` -> `updateDataset`
  - `datalens_delete_dataset` -> `deleteDataset`
  - `datalens_validate_dataset` -> `validateDataset`
  - `datalens_get_entries_relations` -> `getEntriesRelations`
  - `datalens_get_entries` -> `getEntries`
  - `datalens_get_ql_chart` -> `getQLChart`
  - `datalens_delete_ql_chart` -> `deleteQLChart`
  - `datalens_get_wizard_chart` -> `getWizardChart`
  - `datalens_delete_wizard_chart` -> `deleteWizardChart`
  - `datalens_get_editor_chart` -> `getEditorChart`
  - `datalens_delete_editor_chart` -> `deleteEditorChart`
  - `datalens_create_editor_chart` -> `createEditorChart`
  - `datalens_update_editor_chart` -> `updateEditorChart`
  - `datalens_get_entries_permissions` -> `getEntriesPermissions`
  - `datalens_get_audit_entries_updates` -> `getAuditEntriesUpdates`
  - `datalens_list_directory` -> `listDirectory`

## API Coverage (Snapshot)

Coverage snapshot date: **February 17, 2026**.

- Typed coverage:
  - This server includes typed wrappers for all DataLens methods listed in the API overview snapshot (`/openapi-ref/`) at the time of implementation.
  - `datalens_list_methods` exposes that same catalog at runtime to MCP agents.
- Forward compatibility:
  - `datalens_rpc` can call methods that may appear later in DataLens API before a dedicated wrapper is added.
- Experimental methods:
  - Methods marked as experimental in DataLens docs are exposed as tools too. Their behavior can change upstream.

Reference docs used for this snapshot include DataLens API pages updated up to **February 4, 2026** (API start) and method pages updated between **June 26, 2025** and **January 16, 2026**.

## Requirements

- Rust stable (if building from source)
- DataLens organization ID
- Yandex Cloud IAM token (or OAuth token accepted by DataLens)

## Get API Credentials (Detailed)

DataLens Public API requires authentication headers and an organization ID.

From Yandex docs:
- Required role for API calls: `datalens.admin` or `datalens.editor`.
- Common required headers include:
  - `x-dl-org-id`
  - `x-dl-api-version`
  - auth token header (`x-dl-auth-token`; this server also sends `x-yacloud-subjecttoken`)

### 1. Get Your DataLens Organization ID (UI Click Path)

Official doc: <https://yandex.cloud/en/docs/organization/operations/organization-get-id>

1. Open Yandex Cloud Console.
2. In the top panel, click your organization name.
3. Click the organization row to open details.
4. Copy the organization ID.

You will use this value as `DATALENS_ORG_ID`.

### 2. Fastest Way to Get a Token (for local/dev use)

Official docs:
- CLI install: <https://yandex.cloud/en/docs/cli/quickstart>
- Create IAM token: <https://yandex.cloud/en/docs/iam/cli-ref/create-token>

1. Install and initialize `yc` CLI (`yc init`).
2. Run:

```bash
yc iam create-token
```

3. Use the output as `YC_IAM_TOKEN`.

Important: IAM tokens expire. Refresh when expired.

### 3. Automation-Friendly Path (Service Account + Key, UI Click Path)

Official docs:
- Create service account: <https://yandex.cloud/en/docs/iam/quickstart-sa>
- Assign role: <https://yandex.cloud/en/docs/iam/operations/sa/assign-role-for-sa>
- Create authorized key: <https://yandex.cloud/en/docs/iam/operations/authentication/manage-authorized-keys>
- Get IAM token for service account: <https://yandex.cloud/en/docs/iam/operations/iam-token/create-for-sa>

1. Open Yandex Cloud Console.
2. Go to **Identity and Access Management** -> **Service accounts**.
3. Click **Create service account**.
4. Set a name, click **Create**.
5. In role assignment, click **Add role**, then grant at least one DataLens API role (`datalens.editor` or `datalens.admin`).
6. Open the created service account.
7. Click **Create new key** -> **Create authorized key**.
8. Click **Create** and download the key file.
9. Exchange this key for an IAM token using the official instructions from the linked doc.

Use resulting values as:
- `DATALENS_ORG_ID`
- `YC_IAM_TOKEN` or `DATALENS_IAM_TOKEN`

### 4. Where Key Setup Is Applied

Installation sections below include platform-specific commands for setting these values on Linux, macOS, and Windows.

## Installation by Platform

Release artifacts are published on GitHub Releases:
<https://github.com/snevolin/datalens-mcp/releases>

### Linux (x86_64, tar.gz)

1. Download the Linux archive from the latest release.
2. Install binary:

```bash
tar -xzf datalens-mcp-<version>-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 datalens-mcp /usr/local/bin/datalens-mcp
```

3. Configure credentials for this platform:

```bash
# persistent
export DATALENS_ORG_ID="<your_org_id>"

# refresh per session (recommended for user tokens)
export YC_IAM_TOKEN="$(yc iam create-token)"
```

Optional persistence for `DATALENS_ORG_ID`:

```bash
echo 'export DATALENS_ORG_ID="<your_org_id>"' >> ~/.bashrc
```

### Fedora Linux (RPM)

1. Download RPM(s) from the release page.
2. Install:

```bash
sudo dnf install ./datalens-mcp-*.rpm
```

3. Configure credentials for this platform:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

### Debian/Ubuntu Linux (DEB)

1. Download `.deb` from the release page.
2. Install:

```bash
sudo apt install ./datalens-mcp_*_amd64.deb
```

3. Verify binary and man page:

```bash
which datalens-mcp
man datalens-mcp
```

4. Configure credentials for this platform:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

### macOS (Apple Silicon, aarch64 tar.gz)

1. Download the macOS archive from the latest release.
2. Install binary:

```bash
tar -xzf datalens-mcp-<version>-aarch64-apple-darwin.tar.gz
sudo install -m 0755 datalens-mcp /usr/local/bin/datalens-mcp
```

3. Configure credentials for this platform:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

Optional persistence for `DATALENS_ORG_ID`:

```bash
echo 'export DATALENS_ORG_ID="<your_org_id>"' >> ~/.zshrc
```

### Windows (MSI or ZIP)

Option A: MSI
1. Download `.msi` from the release page.
2. Run installer.

Option B: ZIP
1. Download `.zip` from the release page.
2. Extract `datalens-mcp.exe`.
3. Put it in a folder on `PATH`.

Configure credentials for this platform (PowerShell):

```powershell
# persistent
setx DATALENS_ORG_ID "<your_org_id>"

# current session
$env:YC_IAM_TOKEN = yc iam create-token
```

### Build from Source (Any Platform)

```bash
git clone https://github.com/snevolin/datalens-mcp.git
cd datalens-mcp
cargo build --release
```

Binary path:
- Linux/macOS: `target/release/datalens-mcp`
- Windows: `target\release\datalens-mcp.exe`

## Run Manually

Linux/macOS:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
datalens-mcp
```

Windows (PowerShell):

```powershell
$env:DATALENS_ORG_ID = "<your_org_id>"
$env:YC_IAM_TOKEN = yc iam create-token
datalens-mcp.exe
```

## Connect as MCP Server

### Codex CLI / VS Code Codex Extension

Add server:

```bash
codex mcp add datalens -- /usr/local/bin/datalens-mcp
```

Verify:

```bash
codex mcp list
codex mcp get datalens --json
```

If your Codex environment does not inherit shell variables, add explicit env values:

```bash
codex mcp remove datalens
codex mcp add datalens \
  --env DATALENS_ORG_ID=<your_org_id> \
  --env YC_IAM_TOKEN=<your_token> \
  -- /usr/local/bin/datalens-mcp
```

Note: if you store a direct token in config, you must update it after expiration.

### Claude Code (CLI)

Official doc: <https://docs.anthropic.com/en/docs/claude-code/mcp>

Add server:

```bash
claude mcp add datalens -- /usr/local/bin/datalens-mcp
```

If needed, pass explicit env values:

```bash
claude mcp add datalens \
  --env DATALENS_ORG_ID=<your_org_id> \
  --env YC_IAM_TOKEN=<your_token> \
  -- /usr/local/bin/datalens-mcp
```

### Claude Desktop

Official docs:
- Claude MCP overview: <https://docs.anthropic.com/en/docs/claude-code/mcp>
- Local server config flow and config file locations: <https://modelcontextprotocol.io/docs/develop/connect-local-servers>

Click path:
1. Open Claude Desktop.
2. Open **Settings**.
3. Open **Developer** tab.
4. Click **Edit Config**.

Config file locations:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

Example config:

```json
{
  "mcpServers": {
    "datalens": {
      "command": "/usr/local/bin/datalens-mcp",
      "args": [],
      "env": {
        "DATALENS_ORG_ID": "<your_org_id>",
        "YC_IAM_TOKEN": "<your_token>"
      }
    }
  }
}
```

For Windows, set `command` to your `.exe` path, for example:
`C:\\Program Files\\datalens-mcp\\datalens-mcp.exe`

## Environment Variables

- `DATALENS_ORG_ID` (required)
- `YC_IAM_TOKEN` or `DATALENS_IAM_TOKEN` (required)
- `DATALENS_BASE_URL` (optional, default `https://api.datalens.tech`)
- `DATALENS_API_VERSION` (optional, default `0`)
- `DATALENS_TIMEOUT_SECONDS` (optional, default `30`)

## Notes

- API docs use both `api.datalens.tech` and `api.datalens.yandex.net` in different places; this server defaults to `api.datalens.tech` but lets you override base URL.
- For long-running setups, prefer service-account-based token flow and rotation automation.

## License

Apache-2.0 (see `LICENSE`).

## Primary References

- DataLens Public API start: <https://yandex.cloud/en/docs/datalens/operations/api-start>
- DataLens OpenAPI reference index: <https://yandex.cloud/en/docs/datalens/openapi-ref/>
- Organization ID: <https://yandex.cloud/en/docs/organization/operations/organization-get-id>
- IAM: service accounts quickstart: <https://yandex.cloud/en/docs/iam/quickstart-sa>
- IAM: assign role to service account: <https://yandex.cloud/en/docs/iam/operations/sa/assign-role-for-sa>
- IAM: manage authorized keys: <https://yandex.cloud/en/docs/iam/operations/authentication/manage-authorized-keys>
- IAM: create token for service account: <https://yandex.cloud/en/docs/iam/operations/iam-token/create-for-sa>
- IAM: create token via CLI (`yc iam create-token`): <https://yandex.cloud/en/docs/iam/cli-ref/create-token>
- Claude Code MCP docs: <https://docs.anthropic.com/en/docs/claude-code/mcp>
- MCP local server connection guide (Claude Desktop config flow): <https://modelcontextprotocol.io/docs/develop/connect-local-servers>
