# datalens-mcp

Rust MCP-сервер для Public API Yandex DataLens (`https://api.datalens.tech`).

Сервер использует MCP-транспорт `stdio` и предоставляет DataLens RPC-методы как MCP tools.

## Поддерживаемые Tools

- Служебные:
  - `datalens_list_methods`: возвращает известные методы DataLens API, соответствующие MCP tools, категории и метаданные снимка.
  - `datalens_rpc`: универсальный fallback для любого метода по пути `/rpc/{method}`.
- Типизированные обёртки (method-specific):
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

## Покрытие API (Снимок)

Дата снимка покрытия: **17 февраля 2026**.

- Типизированное покрытие:
  - Сервер содержит типизированные обёртки для всех методов DataLens из snapshot API overview (`/openapi-ref/`) на момент реализации.
  - `datalens_list_methods` отдаёт этот же каталог во время работы MCP-сервера.
- Forward compatibility:
  - `datalens_rpc` может вызывать методы, которые появятся позже в DataLens API до добавления отдельной обёртки.
- Экспериментальные методы:
  - Методы, отмеченные в документации DataLens как experimental, также доступны как tools. Их поведение может измениться upstream.

Для этого снимка использованы страницы документации DataLens API, обновлённые до **4 февраля 2026** (старт API), и страницы методов, обновлённые в период **26 июня 2025** — **16 января 2026**.

## Требования

- Rust stable (если собираете из исходников)
- ID организации DataLens
- IAM-токен Yandex Cloud (или OAuth-токен, который принимает DataLens)

## Получение Доступа к API (Подробно)

Public API DataLens требует auth-заголовки и ID организации.

Из документации Yandex:
- Нужная роль для API-запросов: `datalens.admin` или `datalens.editor`.
- Основные заголовки:
  - `x-dl-org-id`
  - `x-dl-api-version`
  - auth-заголовок (`x-dl-auth-token`; этот сервер также отправляет `x-yacloud-subjecttoken`)

### 1. Получить ID Организации DataLens (Путь по кнопкам в UI)

Официальная документация: <https://yandex.cloud/ru/docs/organization/operations/organization-get-id>

1. Откройте Yandex Cloud Console.
2. В верхней панели нажмите на название организации.
3. Нажмите на строку организации, чтобы открыть детали.
4. Скопируйте ID организации.

Это значение используйте как `DATALENS_ORG_ID`.

### 2. Самый Быстрый Способ Получить Токен (для local/dev)

Официальная документация:
- Установка CLI: <https://yandex.cloud/ru/docs/cli/quickstart>
- Получение IAM-токена: <https://yandex.cloud/ru/docs/iam/cli-ref/create-token>

1. Установите и инициализируйте `yc` CLI (`yc init`).
2. Выполните:

```bash
yc iam create-token
```

3. Используйте вывод как `YC_IAM_TOKEN`.

Важно: IAM-токены имеют срок действия. Обновляйте токен после истечения.

### 3. Путь для Автоматизации (Service Account + Key, Путь по кнопкам в UI)

Официальная документация:
- Создание service account: <https://yandex.cloud/ru/docs/iam/quickstart-sa>
- Назначение роли: <https://yandex.cloud/ru/docs/iam/operations/sa/assign-role-for-sa>
- Создание авторизованного ключа: <https://yandex.cloud/ru/docs/iam/operations/authentication/manage-authorized-keys>
- Получение IAM-токена для service account: <https://yandex.cloud/ru/docs/iam/operations/iam-token/create-for-sa>

1. Откройте Yandex Cloud Console.
2. Перейдите в **Identity and Access Management** -> **Service accounts**.
3. Нажмите **Create service account**.
4. Укажите имя, нажмите **Create**.
5. В назначении ролей нажмите **Add role** и выдайте минимум одну роль DataLens API (`datalens.editor` или `datalens.admin`).
6. Откройте созданный service account.
7. Нажмите **Create new key** -> **Create authorized key**.
8. Нажмите **Create** и скачайте файл ключа.
9. Обменяйте этот ключ на IAM-токен по официальной инструкции из ссылки выше.

Итоговые значения используйте как:
- `DATALENS_ORG_ID`
- `YC_IAM_TOKEN` или `DATALENS_IAM_TOKEN`

### 4. Где Применяются Эти Ключи

В разделах установки ниже есть платформенные команды для настройки этих значений в Linux, macOS и Windows.

## Установка по Платформам

Артефакты сборок публикуются в GitHub Releases:
<https://github.com/snevolin/datalens-mcp/releases>

### Linux (x86_64, tar.gz)

1. Скачайте Linux-архив из последнего релиза.
2. Установите бинарник:

```bash
tar -xzf datalens-mcp-<version>-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 datalens-mcp /usr/local/bin/datalens-mcp
```

3. Настройте ключи для этой платформы:

```bash
# постоянно
export DATALENS_ORG_ID="<your_org_id>"

# обновлять в каждой сессии (рекомендуется для user-токенов)
export YC_IAM_TOKEN="$(yc iam create-token)"
```

Опционально для постоянного `DATALENS_ORG_ID`:

```bash
echo 'export DATALENS_ORG_ID="<your_org_id>"' >> ~/.bashrc
```

### Fedora Linux (RPM)

1. Скачайте RPM из страницы релиза.
2. Установите:

```bash
sudo dnf install ./datalens-mcp-*.rpm
```

3. Настройте ключи для этой платформы:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

### Debian/Ubuntu Linux (DEB)

1. Скачайте `.deb` со страницы релиза.
2. Установите:

```bash
sudo apt install ./datalens-mcp_*_amd64.deb
```

3. Проверьте бинарник и man-страницу:

```bash
which datalens-mcp
man datalens-mcp
```

4. Настройте ключи для этой платформы:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

### macOS (Apple Silicon, aarch64 tar.gz)

1. Скачайте macOS-архив из последнего релиза.
2. Установите бинарник:

```bash
tar -xzf datalens-mcp-<version>-aarch64-apple-darwin.tar.gz
sudo install -m 0755 datalens-mcp /usr/local/bin/datalens-mcp
```

3. Настройте ключи для этой платформы:

```bash
export DATALENS_ORG_ID="<your_org_id>"
export YC_IAM_TOKEN="$(yc iam create-token)"
```

Опционально для постоянного `DATALENS_ORG_ID`:

```bash
echo 'export DATALENS_ORG_ID="<your_org_id>"' >> ~/.zshrc
```

### Windows (MSI или ZIP)

Вариант A: MSI
1. Скачайте `.msi` со страницы релиза.
2. Запустите установщик.

Вариант B: ZIP
1. Скачайте `.zip` со страницы релиза.
2. Распакуйте `datalens-mcp.exe`.
3. Положите бинарник в папку, добавленную в `PATH`.

Настройте ключи для этой платформы (PowerShell):

```powershell
# постоянно
setx DATALENS_ORG_ID "<your_org_id>"

# текущая сессия
$env:YC_IAM_TOKEN = yc iam create-token
```

### Сборка из Исходников (Любая Платформа)

```bash
git clone https://github.com/snevolin/datalens-mcp.git
cd datalens-mcp
cargo build --release
```

Путь к бинарнику:
- Linux/macOS: `target/release/datalens-mcp`
- Windows: `target\release\datalens-mcp.exe`

## Ручной Запуск

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

## Подключение как MCP Server

### Codex CLI / VS Code Codex Extension

Добавить сервер:

```bash
codex mcp add datalens -- /usr/local/bin/datalens-mcp
```

Проверка:

```bash
codex mcp list
codex mcp get datalens --json
```

Если среда Codex не наследует shell-переменные, добавьте env-значения явно:

```bash
codex mcp remove datalens
codex mcp add datalens \
  --env DATALENS_ORG_ID=<your_org_id> \
  --env YC_IAM_TOKEN=<your_token> \
  -- /usr/local/bin/datalens-mcp
```

Примечание: если токен сохранён в конфиге напрямую, его надо обновлять после истечения.

### Claude Code (CLI)

Официальная документация: <https://docs.anthropic.com/en/docs/claude-code/mcp>

Добавить сервер:

```bash
claude mcp add datalens -- /usr/local/bin/datalens-mcp
```

При необходимости можно передать env-значения явно:

```bash
claude mcp add datalens \
  --env DATALENS_ORG_ID=<your_org_id> \
  --env YC_IAM_TOKEN=<your_token> \
  -- /usr/local/bin/datalens-mcp
```

### Claude Desktop

Официальная документация:
- Обзор MCP для Claude: <https://docs.anthropic.com/en/docs/claude-code/mcp>
- Поток настройки local server и расположения config-файлов: <https://modelcontextprotocol.io/docs/develop/connect-local-servers>

Путь по кнопкам:
1. Откройте Claude Desktop.
2. Откройте **Settings**.
3. Откройте вкладку **Developer**.
4. Нажмите **Edit Config**.

Расположение config-файлов:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

Пример config:

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

Для Windows укажите в `command` путь к `.exe`, например:
`C:\\Program Files\\datalens-mcp\\datalens-mcp.exe`

## Переменные Окружения

- `DATALENS_ORG_ID` (обязательно)
- `YC_IAM_TOKEN` или `DATALENS_IAM_TOKEN` (обязательно)
- `DATALENS_BASE_URL` (опционально, по умолчанию `https://api.datalens.tech`)
- `DATALENS_API_VERSION` (опционально, по умолчанию `0`)
- `DATALENS_TIMEOUT_SECONDS` (опционально, по умолчанию `30`)

## Примечания

- В документации API встречаются оба домена `api.datalens.tech` и `api.datalens.yandex.net`; сервер по умолчанию использует `api.datalens.tech`, но базовый URL можно переопределить.
- Для long-running setup лучше использовать flow с service account и автоматическим обновлением токенов.

## Лицензия

Apache-2.0 (см. `LICENSE`).

## Основные Источники

- Старт Public API DataLens: <https://yandex.cloud/ru/docs/datalens/operations/api-start>
- Индекс OpenAPI DataLens: <https://yandex.cloud/ru/docs/datalens/openapi-ref/>
- ID организации: <https://yandex.cloud/ru/docs/organization/operations/organization-get-id>
- IAM: quickstart по service account: <https://yandex.cloud/ru/docs/iam/quickstart-sa>
- IAM: назначить роль service account: <https://yandex.cloud/ru/docs/iam/operations/sa/assign-role-for-sa>
- IAM: управление авторизованными ключами: <https://yandex.cloud/ru/docs/iam/operations/authentication/manage-authorized-keys>
- IAM: получить токен для service account: <https://yandex.cloud/ru/docs/iam/operations/iam-token/create-for-sa>
- IAM: получить токен через CLI (`yc iam create-token`): <https://yandex.cloud/ru/docs/iam/cli-ref/create-token>
- Документация Claude Code MCP: <https://docs.anthropic.com/en/docs/claude-code/mcp>
- Гайд по подключению local server (flow Claude Desktop): <https://modelcontextprotocol.io/docs/develop/connect-local-servers>
