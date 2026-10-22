# 数据库

客户端为纯单机应用，数据存本地 SQLite（rusqlite bundled），库文件位于系统应用数据目录（Windows: `%APPDATA%/com.bluef1ash.dealerintegration/dealer-integration.db`），由 `src-tauri/src/db.rs` 在应用启动时打开、执行建表与渐进迁移（`ALTER TABLE` 补列，幂等）。**全部凭证均存 SQLite，不依赖系统凭证库**（用户决策 2026-10-22：跨平台使用，凭证随库迁移）：会话 cookie 与巨量 token 在 `credentials` 键值表，巨懂车密码在 `stores.password`，巨量应用凭证在 `ocean_apps`。跨设备迁移直接拷贝库文件即可；自旧版（系统凭证库存凭证）升级不自动迁移，需重新登录门店并重新录入/授权巨量应用。

## 表结构

### settings（键值设置表）

| key | value | 写入方 |
| --- | --- | --- |
| `snapshot.hour` | 每日对账快照触发小时（0-23，默认 23） | 设置页 |
| `scheduler.enabled` | 自动拉取开关（`1`/`0`，默认开） | 设置页 |
| `scheduler.interval_minutes` | 自动拉取间隔分钟（15-1440，默认 60） | 设置页 |

历史键 `ocean.app_id` / `ocean.app_secret`（旧版单应用凭证）在启动迁移 `repo::ocean_apps::migrate_legacy_db` 中消费后即删除，不再使用。

```sql
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### credentials（凭证键值表）

| name | value | 写入方 |
| --- | --- | --- |
| `store:{account}:cookie` | 巨懂车会话整罐（单条存储） | 登录窗口捕获 |
| `ocean:{app_id}:access_token` / `refresh_token` | 巨量 OAuth token | 授权 / 调度器刷新 |

```sql
CREATE TABLE IF NOT EXISTS credentials (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### stores（门店）

```sql
CREATE TABLE IF NOT EXISTS stores (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    account    TEXT NOT NULL UNIQUE,   -- 巨懂车账号（手机号）
    password   TEXT NOT NULL DEFAULT '', -- 巨懂车密码（明文，供自动重登）
    created_at INTEGER NOT NULL
);
```

### leads（线索，主键 = 列表接口 leads_id）

| 列 | 说明 |
| --- | --- |
| `leads_id` TEXT PK | 列表接口线索 ID（约 10 位） |
| `store_id` → stores | 归属门店（以该店会话拉取） |
| `phone` / `name` / `status_key` / `status_label` / `source_label` / `intention` | 解析字段（`source_label` 即线索类型：私聊线索/广告直接线索） |
| `leads_time` INTEGER | 留资时间 unix 秒 |
| `feiyu_leads_id` TEXT | 飞鱼线索 ID（19 位，导出表「线索ID」列同源；跨表合并键） |
| `ad_id` / `promotion_id` / `promotion_name` | 广告归因（列表接口 `extra` 原生携带） |
| `city` TEXT | 询价城市（省/市；列表取 `city_info`+`phone_city`，导出「询价城市」同口径） |
| `traffic_type` TEXT | 流量类型（自然经营线索/广告直接线索；列表取 `sub_product_type`，导出「流量类型」） |
| `conversion_status` TEXT | 转化状态（私信/表单等转化动作；列表取 `feiyu_action_type`，导出「转化状态」） |
| `scene` TEXT | 来源场景（一级[/二级]；列表取 `from_scene`+`from_sub_scene`，导出「一级/二级来源场景」） |
| `raw` TEXT | 原始 JSON 兜底 |

```sql
CREATE TABLE IF NOT EXISTS leads (
    leads_id       TEXT PRIMARY KEY,
    store_id       INTEGER NOT NULL REFERENCES stores(id),
    phone          TEXT NOT NULL DEFAULT '',
    name           TEXT NOT NULL DEFAULT '',
    status_key     TEXT NOT NULL DEFAULT '',
    status_label   TEXT NOT NULL DEFAULT '',
    source_label   TEXT NOT NULL DEFAULT '',
    intention      TEXT NOT NULL DEFAULT '',
    leads_time     INTEGER NOT NULL,
    feiyu_leads_id TEXT NOT NULL DEFAULT '',
    ad_id          TEXT NOT NULL DEFAULT '',
    promotion_id   TEXT NOT NULL DEFAULT '',
    promotion_name TEXT NOT NULL DEFAULT '',
    city              TEXT NOT NULL DEFAULT '',
    traffic_type      TEXT NOT NULL DEFAULT '',
    conversion_status TEXT NOT NULL DEFAULT '',
    scene             TEXT NOT NULL DEFAULT '',
    raw            TEXT NOT NULL DEFAULT '{}'
);
```

### store_pull_state（采集状态：角标与会话恢复的依据）

```sql
CREATE TABLE IF NOT EXISTS store_pull_state (
    store_id        INTEGER PRIMARY KEY REFERENCES stores(id),
    last_success_at INTEGER,
    last_attempt_at INTEGER,
    last_error      TEXT NOT NULL DEFAULT '',
    backfilled      INTEGER NOT NULL DEFAULT 0   -- 是否已完成 90 天首次回填
);
```

### ocean_apps（巨量引擎应用，多应用管理）

```sql
CREATE TABLE IF NOT EXISTS ocean_apps (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL,
    app_id           TEXT NOT NULL UNIQUE,   -- 商业开放平台 App ID（数字串）
    app_secret       TEXT NOT NULL,
    token_expires_at INTEGER,                -- OAuth token 过期时间 unix 秒；NULL = 从未授权
    created_at       INTEGER NOT NULL
);
```

每个应用独立 OAuth 授权；token 本体按 `ocean:{app_id}:access_token` / `ocean:{app_id}:refresh_token` 分条存 `credentials` 表。调度器逐应用刷新（过期前 5 分钟）并拉取其名下账户的日报，未授权应用跳过。

### ad_accounts / store_ad_accounts（广告账户与门店多对多映射）

```sql
CREATE TABLE IF NOT EXISTS ad_accounts (
    id           TEXT PRIMARY KEY,    -- 巨量广告账户 ID
    alias        TEXT NOT NULL DEFAULT '',
    ocean_app_id INTEGER REFERENCES ocean_apps(id),  -- 归属应用；NULL = 来源应用已删除（报表保留，仅不再刷新）
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS store_ad_accounts (
    store_id      INTEGER NOT NULL REFERENCES stores(id),
    ad_account_id TEXT NOT NULL REFERENCES ad_accounts(id),
    PRIMARY KEY (store_id, ad_account_id)
);
```

### ad_reports_daily / plan_reports_daily（巨量报表，按日 upsert）

```sql
CREATE TABLE IF NOT EXISTS ad_reports_daily (
    ad_account_id TEXT NOT NULL REFERENCES ad_accounts(id),
    date          TEXT NOT NULL,          -- YYYY-MM-DD
    spend         REAL NOT NULL DEFAULT 0,
    impressions   INTEGER NOT NULL DEFAULT 0,
    clicks        INTEGER NOT NULL DEFAULT 0,
    conversions   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ad_account_id, date)
);

CREATE TABLE IF NOT EXISTS plan_reports_daily (
    ad_account_id  TEXT NOT NULL REFERENCES ad_accounts(id),
    promotion_id   TEXT NOT NULL,
    promotion_name TEXT NOT NULL DEFAULT '',
    date           TEXT NOT NULL,
    spend          REAL NOT NULL DEFAULT 0,
    conversions    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ad_account_id, promotion_id, date)
);
```

### snapshots（对账快照登记，一天一条）

```sql
CREATE TABLE IF NOT EXISTS snapshots (
    date        TEXT PRIMARY KEY,   -- YYYY-MM-DD
    imported_at INTEGER NOT NULL,
    leads_seen  INTEGER NOT NULL DEFAULT 0
);
```

## 合并口径

- 线索：列表拉取与快照合并统一按 `leads_id` upsert；快照行先按 `feiyu_leads_id` 找回列表主键（导出表线索 ID 与列表 ID **不是同一 ID 空间**，见 GLOSSARY「线索 ID 双口径」与 ADR-0003 验证结论）。
- 报表：`(ad_account_id[, promotion_id], date)` upsert，重拉覆盖。
- 快照登记：`snapshots` 按日 upsert，重跑不重复导入。

## 测试夹具

`src-tauri/tests/fixtures/judongche/`（列表 JSON + 导出 xlsx）与 `oceanengine/`（报表样例）为脱敏真实响应，供入库/出库接缝回放测试使用，契约说明见该目录 README。
