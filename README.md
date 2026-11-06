# Space API (Rust Edition)

> 天翔TNXGの空间站 API - 高性能 Rust 后端实现

![Rust](https://img.shields.io/badge/Rust-Modren-orange?style=flat-square&logo=rust)
![Rocket](https://img.shields.io/badge/Rocket-v0.5-red?style=flat-square&logo=rust)
![MongoDB](https://img.shields.io/badge/MongoDB-Driver-green?style=flat-square&logo=mongodb)
![License](https://img.shields.io/badge/License-AGPLv3-blue?style=flat-square)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/TNXG/space-api)

Space API 是一个基于 Rust 语言和 Rocket 框架构建的高性能、异步 RESTful API 服务。它为个人空间站（Blog/Portfolio）提供后端支持，涵盖用户管理、OAuth 认证、邮件服务、状态监控等核心功能。

## ✨ 特性

- **高性能核心**：基于 Rust 语言，利用其零成本抽象和内存安全特性，提供极致的性能表现。
- **全异步架构**：使用 Tokio 运行时和 Rocket 的异步处理能力，轻松应对高并发请求。
- **智能内存管理**：集成 jemalloc 内存分配器，自动监控内存使用并在超过阈值时执行全局内存释放。
- **实时系统监控**：通过 Server-Sent Events (SSE) 推送 CPU 使用率、内存分配和压力等级等实时指标。
- **模块化设计**：路由、服务、模型分层清晰，易于维护和扩展。
- **OAuth 集成**：内置 QQ 等第三方登录支持，简化用户认证流程。
- **邮件服务**：基于 Lettre 库实现的异步邮件发送功能，支持 SMTP 协议。
- **图片处理**：集成 Image 库，支持图片上传、处理和转换。
- **状态监控**：实时监控服务器运行状态和 API 健康状况。
- **安全可靠**：严格的类型系统和错误处理机制，确保服务稳定运行。

## 🛠 技术栈

| 组件 | 技术选型 | 说明 |
| :--- | :--- | :--- |
| **语言** | [Rust](https://www.rust-lang.org/) | 2021 Edition |
| **Web 框架** | [Rocket](https://rocket.rs/) | v0.5.1, 简单、极速、类型安全 |
| **数据库** | [MongoDB](https://www.mongodb.com/) | NoSQL 数据库，搭配官方 Rust Driver |
| **异步运行时** | [Tokio](https://tokio.rs/) | Rust 生态事实标准的异步运行时 |
| **内存分配器** | [jemalloc](https://jemalloc.net/) | 高性能内存分配器，支持内存统计和释放 |
| **序列化** | [Serde](https://serde.rs/) | 高效的序列化/反序列化框架 |
| **HTTP 客户端** | [Reqwest](https://docs.rs/reqwest/) | 强大的异步 HTTP 客户端 |
| **缓存** | [Moka](https://github.com/moka-rs/moka) | 高性能、并发缓存库 |
| **邮件** | [Lettre](https://lettre.rs/) | 强类型的邮件构建和传输库 |
| **系统监控** | [Sysinfo](https://docs.rs/sysinfo/) | 跨平台系统信息获取库 |
| **模板引擎** | Tera | Rocket 集成的动态模板引擎 |

## 📂 项目结构

```
space-api/
├── src/
│   ├── config/         # 配置管理模块
│   ├── models/         # 数据库模型定义 (Structs & Schemas)
│   ├── routes/         # API 路由处理层
│   │   ├── admin.rs    # 管理员相关路由
│   │   ├── auth.rs     # 认证相关路由
│   │   ├── index.rs    # 首页和 SSE 监控路由
│   │   └── ...
│   ├── services/       # 业务逻辑服务层 (DB操作等)
│   │   ├── memory_service.rs  # 内存管理服务
│   │   └── ...
│   ├── templates/      # Tera 模板文件
│   ├── utils/          # 工具函数库
│   │   ├── jemalloc_interface.rs  # jemalloc 集成接口
│   │   └── ...
│   └── main.rs         # 程序入口与应用配置
├── Cargo.toml          # 依赖管理文件
├── Rocket.toml         # Rocket 框架配置文件
├── config.toml         # 应用程序配置文件
└── config.example.toml # 配置文件示例
```

## 🚀 快速开始

### 前置要求

- [Rust Toolchain](https://www.rust-lang.org/tools/install) (建议最新稳定版)
- MongoDB 实例 (本地或远程)

### 安装与运行

1.  **克隆项目**

    ```bash
    git clone https://github.com/your-username/space-api-rs.git
    cd space-api-rs
    ```

2.  **配置环境**

    复制示例配置文件并根据实际情况修改：

    ```bash
    cp config.example.toml config.toml
    ```

    编辑 `config.toml` 文件，填入必要的配置信息：

    ```toml
    [mongo]
    host = "localhost"
    port = 27017
    database = "space-api"
    
    [email]
    smtp_server = "smtp.example.com"
    smtp_port = 465
    username = "your_email@example.com"
    password = "your_password"
    from_address = "noreply@example.com"
    from_name = "Space API"
    
    [oauth]
    qq_app_id = "your_qq_app_id"
    qq_app_key = "your_qq_app_key"
    redirect_uri = "https://your-domain.com/oauth/qq/callback"

    [image_proxy]
    allowed_referrer_domains = ["tnxg.moe", "tnxg.top"]
    allow_missing_referrer = false
    max_image_bytes = 10485760
    
    [memory]
    threshold_mb = 500
    check_interval_secs = 30
    gc_cooldown_secs = 30
    ```

3.  **运行服务**

    使用 Cargo 启动开发服务器：

    ```bash
    cargo run
    ```

    ```bash
    cargo build --release
    ./target/release/space-api-rs
    ```

4.  **Docker 部署**

    项目提供了 `Dockerfile` 和 `docker-compose.yml`，可一键部署：

    ```bash
    # 使用 Docker Compose 启动 (需先配置 config.toml)
    docker-compose up -d
    ```

## ⚙️ 配置说明

项目使用 TOML 文件进行配置，默认加载运行目录下的 `config.toml`。环境变量可以作为覆盖项（优先级：环境变量 > 配置文件）。

### 配置文件示例 (`config.toml`)

```toml
# Space API Configuration Example

[mongo]
host = "localhost"            # MongoDB 主机地址
port = 27017                  # MongoDB 端口
# user = "username"           # 可选：MongoDB 用户名（如未启用认证可留空）
# password = "password"       # 可选：MongoDB 密码
database = "space-api"        # 要连接的数据库名

[email]
smtp_server = "smtp.example.com"      # SMTP 服务器地址
smtp_port = 465                       # SMTP 端口（465=SSL，587=TLS）
username = "noreply@example.com"      # SMTP 登录用户名（通常是邮箱）
password = "your_email_password"      # SMTP 授权密码或应用专用密码
from_address = "noreply@example.com"  # 发件人邮箱地址
from_name = "Space API"               # 发件人显示名称

[oauth]
qq_app_id = "your_qq_app_id"                # QQ 登录 AppID
qq_app_key = "your_qq_app_key"              # QQ 登录 AppKey
redirect_uri = "https://your-domain.com/oauth/qq/callback"  # 回调地址

[image_proxy]
allowed_referrer_domains = ["tnxg.moe", "tnxg.top"] # 防盗链白名单，自动允许子域名
allow_missing_referrer = false                 # 是否允许缺少来源头的请求
max_image_bytes = 10485760                     # 单张源图最大 10 MiB

[memory]
threshold_mb = 500              # 内存阈值（MB），超过此值触发全局内存释放
check_interval_secs = 30        # 内存检查间隔（秒），设置过小会增加 CPU 负担
gc_cooldown_secs = 30           # GC 最小间隔（秒），避免频繁触发垃圾回收

# Why TOML?
# 1. 语法简单、结构清晰，适合手写配置。
# 2. 强类型（整数、布尔、字符串等）减少解析歧义。
# 3. 支持自然的分段结构（如 [mongo]、[email]），便于组织配置。
# 4. Rust 社区广泛支持（如 toml、config crates），生态成熟，集成方便。
# 5. 与 JSON 相比更适合配置文件；与 YAML 相比更少坑、更稳定。
```

### 环境变量覆盖

可以通过 `SPACE_API` 前缀的环境变量覆盖配置。层级使用双下划线 `__` 分隔：

- `SPACE_API_MONGO__HOST` 覆盖 configuration `[mongo] host`
- `SPACE_API_EMAIL__PASSWORD` 覆盖 configuration `[email] password`
- `SPACE_API_MEMORY__THRESHOLD_MB` 覆盖 configuration `[memory] threshold_mb`

### 内存管理功能

Space API 集成了智能内存管理系统，具备以下特性：

- **自动内存监控**：后台任务定期检查内存使用情况
- **智能内存释放**：当内存使用超过配置阈值时自动执行全局内存释放
- **压力等级计算**：根据内存使用百分比计算压力等级（低/中等/高/严重）
- **实时监控界面**：通过 SSE 向前端推送实时的 CPU 和内存使用数据
- **jemalloc 集成**：利用 jemalloc 内存分配器的高级功能进行内存优化

访问 `/` 路径可查看实时系统监控界面，包含：
- CPU 使用率实时图表
- 内存分配量和使用百分比
- 内存压力等级指示器
- 历史数据趋势图

## 🔌 API 概览

| 模块 | 路径前缀 | 描述 |
| :--- | :--- | :--- |
| **Index** | `/` | 服务基础信息与实时系统监控界面 |
| **Metrics** | `/metrics` | SSE 实时系统指标推送 |
| **User** | `/user` | 用户注册、登录、信息查询 |
| **Avatar** | `/avatar` | 头像上传与获取 |
| **Email** | `/email` | 邮件发送服务 |
| **Images** | `/images` | 图片管理接口 |
| **Image Proxy** | `/images/proxy` | 带缓存、格式协商与防盗链的通用图像代理 |
| **Links** | `/links` | 友链/链接管理 |
| **OAuth** | `/oauth` | 第三方登录回调处理 |
| **Status** | `/status` | 系统运行状态检查 |

### 随机壁纸

`GET /images/wallpaper`（横图）、`GET /images/wallpaper_height`（竖图）共享以下参数：

| 参数 | 可选值 | 默认 |
| :--- | :--- | :--- |
| `mode` / `m` | `proxy` / `p`、`redirect` / `r`、`json` / `j` | `proxy` |
| `source` / `s` | `cdn` / `c`、`cdcn` / `cn` | `cdn` |
| `format` / `f` | `original` / `o`、`jxl` / `j` | `original` |

- `proxy` 从所选域名原样流式转发，使用有界缓冲提供背压，不等待整图下载、不重编码。
- `redirect` 返回 302；`source=cdn` 使用 `cdn.tnxg.top`，`source=cdcn` 使用 `cdcn.tnxg.top`。
- `format=jxl` 选择同目录、同主名的 `.jxl` 旁路文件；原图保留实际扩展名。上游必须已部署对应文件。
- `json` 一次随机选取一条 blurhash 索引记录，始终返回 `filename`（不含扩展名）、`blurhash`、`original.cdn`、`original.cdcn`、`jxl.cdn`、`jxl.cdcn`。合法的 `source`、`format` 不会裁剪 JSON 字段。
- 不根据 `Accept` 自动切换格式。无效参数返回 400；上游获取失败返回错误，不自动切换来源。
- 随机结果响应使用 `Cache-Control: no-store`，防止随机结果被缓存固定。

兼容旧参数：未提供 `mode` 时，`type=cdn` 等价于 `mode=redirect`，`type=json` 等价于 `mode=json`。
`type=json` 额外保留 `data.image`（CDN 原图地址），已有 `data.blurhash` 不变。旧 `t=cdn/json` 别名同样保留，`type` 优先于 `t`。
优先级为 `mode` > `m` > `type` > `t`；`source` 优先于 `s`，`format` 优先于 `f`。
长短参数和值可混用；长参数显式传入非法值仍返回 400，不会尝试短参数回退。

示例：

```text
/images/wallpaper?mode=json
/images/wallpaper?mode=redirect&source=cdcn&format=jxl
/images/wallpaper_height?mode=proxy&source=cdn&format=jxl
/images/wallpaper?type=cdn
/images/wallpaper?m=j
/images/wallpaper?m=r&s=cn&f=jxl
```

### 模块和测试布局

生产模块按职责拆分：`routes/images/wallpaper.rs` 负责随机壁纸，
`services/image_proxy_service/` 负责图像代理的下载、缓存和请求策略，
`services/memory_service/` 负责内存模型、采集、监控、释放和报告，
`routes/index/` 与 `routes/status/` 存放拆分后的端点。

全部测试实现位于 `tests/`：`tests/wallpaper_routes.rs` 覆盖 HTTP 参数契约，
`tests/unit/` 存放单元测试。源码仅保留 `#[cfg(test)]` 的模块引用，
保持私有接口可测试而不扩大生产 API 的可见性。运行 `cargo test` 执行全部测试。


## 📄 开源协议

本项目采用 **GNU Affero General Public License v3.0 (AGPL-3.0)** 协议开源。
这意味着如果您在服务端运行修改后的版本，必须向所有通过网络与该程序交互的用户公开源代码。

Copyright (c) 2025 Tianxiang TNXG
