# datalens-mcp

Rust MCP server for the Yandex DataLens Public API (`https://api.datalens.tech`).

This server uses the MCP `stdio` transport and exposes DataLens RPC methods as MCP tools.

## Quick Start

Where to start with `datalens-mcp`:

1. Install the MCP server:
   - [Linux (x86_64, tar.gz)](#install-linux-targz)
   - [Fedora Linux (RPM)](#install-fedora-rpm)
   - [Debian/Ubuntu Linux (DEB)](#install-debian-ubuntu-deb)
   - [macOS (Apple Silicon, aarch64 tar.gz)](#install-macos)
   - [Windows (MSI or ZIP)](#install-windows)
   - [Build from source](#install-build-from-source)
2. Connect it to the agents you use:
   - [Codex CLI / VS Code Codex Extension](#connect-codex)
   - [Cursor](#connect-cursor)
   - [Claude Code (CLI)](#connect-claude-code)
   - [Claude Desktop](#connect-claude-desktop)
3. Run first requests:
   - [Usage examples](#usage-examples)

## Disclaimer

- This is an unofficial, community-maintained project and is not affiliated with, sponsored by, or endorsed by Yandex.
- Yandex and DataLens are trademarks of their respective owners.
- The name `datalens-mcp` is used only to describe compatibility with the DataLens API.

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

## API Coverage

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

## Get API Credentials

DataLens Public API requires authentication headers and an organization ID.

From Yandex docs:
- Required role for API calls: `datalens.admin` or `datalens.editor`.
- Common required headers include:
  - `x-dl-org-id`
  - `x-dl-api-version`
  - auth token header (`x-dl-auth-token`; this server also sends `x-yacloud-subjecttoken`)

Choose one token path: section 2 (`yc` CLI), section 3 (OAuth -> IAM), or section 4 (service account).  
All of them must end with an IAM token in `YC_IAM_TOKEN` (or `DATALENS_IAM_TOKEN`).

### 1. Get Your DataLens Organization ID

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

### 3. Alternative Without YC CLI (OAuth -> IAM token)

Official docs:
- Account IAM token (OAuth exchange): <https://yandex.cloud/en/docs/iam/operations/iam-token/create>
- IAM API method (`iam/v1/tokens`): <https://yandex.cloud/en/docs/iam/api-ref/IamToken/create>

Use this path if you do not want to install `yc` locally.

1. Sign in to your Yandex account.
2. Open Yandex OAuth, click **Allow**, and copy the OAuth token:
   - <https://oauth.yandex.com>
3. Exchange OAuth token for IAM token:

```bash
curl \
  --request POST \
  --header 'Content-Type: application/json' \
  --data '{"yandexPassportOauthToken":"<OAuth_token>"}' \
  https://iam.api.cloud.yandex.net/iam/v1/tokens
```

4. From the JSON response, take `iamToken` and set it as `YC_IAM_TOKEN`:

```bash
export YC_IAM_TOKEN="<iam_token>"
```

PowerShell variant:

```powershell
$yandexPassportOauthToken = "<OAuth_token>"
$Body = @{ yandexPassportOauthToken = "$yandexPassportOauthToken" } | ConvertTo-Json -Compress
$env:YC_IAM_TOKEN = (Invoke-RestMethod -Method 'POST' -Uri 'https://iam.api.cloud.yandex.net/iam/v1/tokens' -Body $Body -ContentType 'Application/json').iamToken
```

Important:
- `OAuth_token` is not the same as `IAM token`.
- For this server, always use the resulting IAM token in `YC_IAM_TOKEN` (or `DATALENS_IAM_TOKEN`).
- IAM tokens expire (up to 12 hours). Refresh when expired.

### 4. Automation-Friendly Path (Service Account + Key)

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

### 5. Where Key Setup Is Applied

Installation sections below include platform-specific commands for setting these values on Linux, macOS, and Windows.

<a id="installation"></a>
## Installation by Platform

Release artifacts are published on [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).

<a id="install-linux-targz"></a>
### Linux (x86_64, tar.gz)

1. Download the Linux archive from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
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

<a id="install-fedora-rpm"></a>
### Fedora Linux (RPM)

1. Download RPM(s) from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
2. Install:

```bash
sudo dnf install ./datalens-mcp-*.rpm
```

3. Configure credentials for this platform:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

<a id="install-debian-ubuntu-deb"></a>
### Debian/Ubuntu Linux (DEB)

1. Download `.deb` from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
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

<a id="install-macos"></a>
### macOS (Apple Silicon, aarch64 tar.gz)

1. Download the macOS archive from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
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

<a id="install-windows"></a>
### Windows (MSI or ZIP)

Option A: MSI
1. Download `.msi` from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
2. Run installer.

Option B: ZIP
1. Download `.zip` from [GitHub Releases](https://github.com/snevolin/datalens-mcp/releases).
2. Extract `datalens-mcp.exe`.
3. Put it in a folder on `PATH`.

Configure credentials for this platform (PowerShell):

```powershell
# persistent
setx DATALENS_ORG_ID "<your_org_id>"

# current session
$env:YC_IAM_TOKEN = yc iam create-token
```

<a id="install-build-from-source"></a>
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

<a id="connect-mcp"></a>
## Connect as MCP Server

<a id="connect-codex"></a>
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

<a id="connect-cursor"></a>
### Cursor

Official docs:
- Cursor MCP overview: <https://docs.cursor.com/context/model-context-protocol>
- MCP configuration (`mcp.json`): <https://docs.cursor.com/context/mcp>

You can configure MCP in:
- Project scope: `.cursor/mcp.json` (shared with this repo)
- User scope: `~/.cursor/mcp.json` (all projects)
- User scope on Windows: `%USERPROFILE%\\.cursor\\mcp.json` (PowerShell: `$HOME\\.cursor\\mcp.json`)

Example config:

```json
{
  "mcpServers": {
    "datalens": {
      "type": "stdio",
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

Validate in Cursor Agent (optional):

```bash
cursor-agent mcp list
cursor-agent mcp list-tools datalens
```

<a id="connect-claude-code"></a>
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

<a id="connect-claude-desktop"></a>
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

<a id="usage-examples"></a>
## Usage Examples

After installation and MCP connection, try these common DataLens tasks in your agent:

1. Inventory dashboards:

```text
Show all dashboards in my workspace with folder, owner, and last update date.
```

2. Audit stale objects:

```text
Find dashboards and datasets that were not updated in the last 90 days.
```

3. Inspect one dashboard:

```text
Open dashboard "<dashboard_id>" and summarize its charts, widgets, and selectors.
```

4. Inspect one dataset:

```text
Open dataset "<dataset_id>" and summarize fields, calculated fields, and joins.
```

5. Check access rights:

```text
Show who can view and edit entry "<entry_id>".
```

6. Run impact analysis before changes:

```text
For dataset "<dataset_id>", list dashboards and charts that depend on it.
```

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
- IAM: create account token from OAuth token: <https://yandex.cloud/en/docs/iam/operations/iam-token/create>
- IAM API: `IamToken/create`: <https://yandex.cloud/en/docs/iam/api-ref/IamToken/create>
- IAM: create token via CLI (`yc iam create-token`): <https://yandex.cloud/en/docs/iam/cli-ref/create-token>
- Claude Code MCP docs: <https://docs.anthropic.com/en/docs/claude-code/mcp>
- MCP local server connection guide (Claude Desktop config flow): <https://modelcontextprotocol.io/docs/develop/connect-local-servers>
- Cursor MCP docs: <https://docs.cursor.com/context/model-context-protocol>
- Cursor MCP config docs: <https://docs.cursor.com/context/mcp>
