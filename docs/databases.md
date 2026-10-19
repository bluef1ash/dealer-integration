# 数据库

客户端为纯单机应用，数据存本地 SQLite，库文件位于系统应用数据目录（Windows: `%APPDATA%/com.bluef1ash.dealerintegration/dealer-integration.db`），由 `src-tauri/src/store.rs` 在应用启动时打开并执行建表。当前仅「设置」表；业务表随 #6（线索入库）、#8（报表入库）落地后在此登记。

## settings（键值设置表）

应用级配置。当前键：

| key | value | 写入方 |
| --- | --- | --- |
| `ocean.app_id` | 巨量引擎商业开放平台 App ID | 设置页 |
| `ocean.app_secret` | 巨量引擎商业开放平台 App Secret | 设置页 |

```sql
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

> 决策记录（2026-10-19，issue #13）：App ID / App Secret 不走系统凭证库，经设置页存本地 SQLite（覆盖 spec #1 原「凭证全走凭证库」决策，仅限这两个字段）。巨懂车密码、会话 cookie、巨量 token 等其余凭证仍按原决策走系统凭证库。
