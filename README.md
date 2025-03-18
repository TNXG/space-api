# Space API

基于 Nitro 构建的 API 服务，提供友链管理、图片处理等功能。

## 技术栈

- **框架**: Nitro (基于 h3 的轻量级服务端框架)
- **数据库**: MongoDB
- **语言**: TypeScript
- **图片处理**: Sharp
- **邮件服务**: Nodemailer
- **开发工具**: ESLint, pnpm

## 环境配置

1. 复制 `.env.template` 到 `.env`
2. 配置以下环境变量：
   ```env
   MONGO_HOST=MongoDB主机地址
   MONGO_PORT=MongoDB端口
   MONGO_USER=MongoDB用户名
   MONGO_PASSWORD=MongoDB密码
   JWT_SECRET=JWT密钥
   CODETIME_SESSION=CodeTime会话ID
   EMAIL_HOST=SMTP服务器地址
   EMAIL_PORT=SMTP服务器端口（默认587）
   EMAIL_USER=SMTP用户名
   EMAIL_PASS=SMTP密码
   ```

## 开发调试

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm dev

# 构建项目
pnpm build

# 预览构建结果
pnpm preview
```

## API 接口

### 友链管理

#### `POST /links/verify` - 发送友链验证码

**请求参数：**
```typescript
{
  email: string  // 接收验证码的邮箱地址
}
```

**响应数据：**
```typescript
{
  code: string;       // 状态码
  status: string;     // 状态：success 或 error
  message: string;    // 响应消息
}
```

**错误类型：**
- `400` - 邮箱地址未提供
- `500` - 验证码生成失败或发送失败

#### `POST /links/submit` - 提交友链

**请求参数：**
```typescript
{
  name: string;        // 网站名称
  url: string;         // 网站地址
  avatar: string;      // 头像URL
  description: string; // 网站描述
  email: string;       // 联系邮箱
  code: string;        // 验证码
  rssurl?: string;     // RSS地址（可选）
  techstack?: string[]; // 技术栈（可选）
}
```

**响应数据：**
```typescript
{
  code: string;       // 状态码
  status: string;     // 状态：success 或 error
  message: string;    // 响应消息
  data?: {           // 成功时返回的数据
    name: string;
    url: string;
    avatar: string;
    description: string;
    state: number;
    created: string;
    rssurl: string;
    techstack: string[];
  }
}
```

**错误类型：**
- `400` - 缺少必填字段
- `401` - 验证码无效
- `409` - URL已存在
- `500` - 数据插入失败

#### `GET /links` - 获取友链列表

**查询参数：**
- `page` - 页码（默认：1）
- `size` - 每页数量（默认：50）

**响应数据：**
```typescript
{
  code: string;       // 状态码
  status: string;     // 状态：success 或 error
  data: Array<{      // 友链列表
    name: string;
    url: string;
    avatar: string;
    description: string;
    state: number;
    created: string;
    rssurl: string;
    techstack: string[];
  }>;
  message: {         // 分页信息
    pagination: {
      total: number;        // 总记录数
      current_page: number; // 当前页码
      total_page: number;   // 总页数
      size: number;         // 每页数量
      has_next_page: boolean;
      has_prev_page: boolean;
    }
  }
}
```

**错误类型：**
- `400` - 无效的页码或大小参数

### 图片服务

#### `GET /avatar` - 获取头像

**查询参数：**
- `s` 或 `source` - 头像来源（可选值：qq/QQ、github/GitHub/gh/GH，默认使用自定义头像）

**响应：**
- 成功：返回图片数据，支持WebP等现代图片格式
- 失败：返回JSON格式错误信息
  ```typescript
  {
    code: string;    // 状态码
    message: string; // 错误信息
    status: string;  // error
  }
  ```

**错误类型：**
- `500` - 获取头像失败

#### `GET /images/wallpaper` - 获取壁纸

**查询参数：**
- `type` 或 `t` - 返回类型
  - `cdn` - 返回CDN直链（302重定向）
  - `json` - 返回JSON格式的图片信息
  - 默认返回图片数据

**响应：**
- `type=cdn`：302重定向到CDN地址
- `type=json`：
  ```typescript
  {
    code: string;       // 状态码
    status: string;     // success
    data: {
      image: string;    // 图片URL
      blurhash: string; // BlurHash编码
    }
  }
  ```
- 默认：返回图片数据

**错误类型：**
- `500` - 获取图片失败

### 状态监控

#### `GET /status` - 获取博主状态信息

**查询参数：**
- `s` 或 `source` - 数据来源（默认：codetime）
- `q` 或 `query` - 查询ID（默认：515522946）
- `sse` - 是否启用服务器发送事件（默认：false）
- `interval` 或 `i` - SSE更新间隔，单位毫秒（默认：5000）

**响应数据：**
```typescript
{
  code: string;      // 状态码
  status: string;    // 状态：success 或 error
  message: string;   // 响应消息
  data?: {          // 状态数据
    id: number;     // 记录ID
    user: {         // 用户信息
      id: string;
      avatar: string;
      name: string;
      active: boolean;
    };
    song?: {        // 当前播放歌曲（如果有）
      name: string;
      transNames: string[];
      alias: string[];
      id: string;
      artists: Array<{
        id: string;
        name: string;
      }>;
      album: {
        name: string;
        id: string;
        image: string;
        publishTime: string;
        artists: Array<{
          id: string;
          name: string;
        }>;
      };
    };
    lastUpdate: string; // 最后更新时间
  }
}
```

**错误类型：**
- `400` - 无效的参数（interval < 1000ms、无效的source或query）

### 其他

#### `GET /` - API根路径
返回API基本信息

#### `GET /sw.js` - Service Worker脚本
返回Service Worker脚本文件
