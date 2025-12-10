# Space API Rust

这是天翔TNXGの空间站API的Rust实现版本，使用Rocket框架构建。

## 功能特性

- 用户认证与管理
- 头像处理与缓存
- 邮件发送与验证
- 网易云音乐API集成
- QQ OAuth登录
- 友情链接管理
- 状态监控

## 项目结构

```
space-api-rs/
├── src/
│   ├── main.rs              # 应用程序入口
│   ├── lib.rs               # 模块注册与导出
│   ├── config/              # 配置模块
│   ├── routes/              # API路由
│   ├── services/            # 业务服务
│   ├── cache/               # 缓存模块
│   ├── utils/               # 工具模块
│   └── models/              # 数据模型
└── Cargo.toml               # 项目依赖
```

## 安装与运行

### 前提条件

- Rust 1.72.0 或更高版本
- MongoDB 数据库
- FFmpeg（用于媒体处理）

### 配置

1. 复制示例环境变量文件并进行设置：

```bash
cp .env.example .env
```

2. 根据你的环境修改 `.env` 文件中的配置。

### 构建与运行

```bash
# 安装依赖并构建
cargo build

# 运行服务器
cargo run
```

默认情况下，API服务器将在 `http://localhost:8000` 运行。

## API 端点

### 基础

- `GET /` - API基本信息

### 用户

- `GET /user/info` - 获取用户信息
- `GET /user/get` - 获取用户列表

### 状态

- `GET /status/ncm` - 获取网易云音乐播放状态
- `GET /status/codetime` - 获取代码时间统计

### 头像

- `GET /avatar` - 获取并处理头像图像

### 图像

- `GET /images/wallpaper` - 获取壁纸图像
- `GET /images/wallpaper_height` - 获取壁纸信息

### 邮件

- `POST /email/send` - 发送验证邮件
- `POST /email/verify` - 验证邮箱

### 链接

- `GET /links` - 获取友情链接列表
- `POST /links/submit` - 提交新链接
- `POST /links/verify` - 验证链接

### OAuth

- `GET /oauth/qq/login` - QQ登录链接
- `GET /oauth/qq/callback` - QQ登录回调

## 测试

运行测试套件：

```bash
cargo test
```

## 性能优化

本项目使用了以下技术进行性能优化：

- 基于Moka的高性能缓存系统
- 异步处理所有IO操作
- 响应式数据处理
- 高效的图像处理

## 许可证

MIT