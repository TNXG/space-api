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
| **Links** | `/links` | 友链/链接管理 |
| **OAuth** | `/oauth` | 第三方登录回调处理 |
| **Status** | `/status` | 系统运行状态检查 |

## 📄 开源协议

本项目采用 **GNU Affero General Public License v3.0 (AGPL-3.0)** 协议开源。
这意味着如果您在服务端运行修改后的版本，必须向所有通过网络与该程序交互的用户公开源代码。

Copyright (c) 2025 Tianxiang TNXG