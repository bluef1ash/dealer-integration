# 数据库

客户端为纯单机应用，数据存本地 SQLite（rusqlite bundled），库文件位于系统应用数据目录（Windows: `%APPDATA%/com.bluef1ash.dealerintegration/dealer-integration.db`），由 `src-tauri/src/db.rs` 在应用启动时打开、执行建表与渐进迁移（`ALTER TABLE` 补列，幂等）。凭证（会话 cookie、账号密码、巨量 token）**不入库**，存系统凭证库（`src-tauri/src/credentials.rs`，Windows 凭据管理器 / macOS 钥匙串）；例外是巨量 App ID / Secret，按用户决策（issue #13）经设置页存 `settings` 表。

## 表结构

### settings（键值设置表）

| key | value | 写入方 |
| --- | --- | --- |
| `ocean.app_id` | 巨量引擎商业开放平台 App ID | 设置页 |
| `ocean.app_secret` | 巨量引擎商业开放平台 App Secret | 设置页 |
| `ocean.token.expires_at` | OAuth token 过期时间（unix 秒） | 调度器 |
| `snapshot.hour` | 每日对账快照触发小时（0-23，默认 23） | 设置页 |

```sql
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### stores（门店）

```sql
CREATE TABLE IF NOT EXISTS stores (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    account    TEXT NOT NULL UNIQUE,   -- 巨懂车账号（手机号）
    created_at INTEGER NOT NULL
);
```

### leads（线索，主键 = 列表接口 leads_id）

| 列 | 说明 |
| --- | --- |
| `leads_id` TEXT PK | 列表接口线索 ID（约 10 位） |
| `store_id` → stores | 归属门店（以该店会话拉取） |
| `phone` / `name` / `status_key` / `status_label` / `source_label` / `intention` | 解析字段 |
| `leads_time` INTEGER | 留资时间 unix 秒 |
| `feiyu_leads_id` TEXT | 飞鱼线索 ID（19 位，导出表「线索ID」列同源；跨表合并键） |
| `ad_id` / `promotion_id` / `promotion_name` | 广告归因（列表接口 `extra` 原生携带） |
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

### ad_accounts / store_ad_accounts（广告账户与门店多对多映射）

```sql
CREATE TABLE IF NOT EXISTS ad_accounts (
    id         TEXT PRIMARY KEY,    -- 巨量广告账户 ID
    alias      TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
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
