# 安全

Zeyn 中的安全设置与它所保护的代码写在一起，并且服务器会在每次调用时检查。默认设置是严格的；放宽任何一项，都会在 `app.rs` 中留下一行可见的代码。

## 权限

每个 action 和 screen 都要声明 `permission = "feature.resource.verb"` 或 `public`。缺少声明则无法编译。请求的身份（identity）携带它所拥有的权限：

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

用户无权运行的菜单项、工具栏按钮、行操作和命令不会出现在 HTML 中。省略它们只是为了方便使用；服务器在每次调用时都会再次检查权限。

## 登录

应用本身不保存密码。由 `Authenticator` 判断调用者是谁，通常来自应用前面的单点登录代理：

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

只有在每个请求都经过代理时才使用 `ForwardAuth`；任何能直接访问应用的人都可以发送这些请求头。`require_header` 会检查代理附加的共享密钥。`DevAuth::superuser(name)` 为本地开发中的每个请求授予完全访问权限，启用时服务器会打印警告。发布构建（release build）会拒绝带着它启动（`App::check` 会报告）；必须在发布版本中运行的演示可以调用 `App::allow_dev_auth()`。使用 `zeyn new` 创建的应用只在调试构建中启用它。

## Guards

guard（守卫）在 Feature 的任何 screen 打开之前，以及它的任何 grid、form 或命令被调用之前运行：

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

内置 guard 有：`SignedIn`、`TenantRequired` 和 `Licensed`。

## Interceptors

interceptor（拦截器）在访问检查之后，围绕每一次 screen、grid、form、API 和命令调用运行。`before` 可以终止调用；`after` 能看到调用如何结束。`AuditLog(sink)` 是内置的拦截器，用于记录谁做了什么。

## CSRF、CSP 与响应头

- 每个页面都会设置一个随机的 `zeyn_csrf` cookie（`HttpOnly`、`SameSite=Strict`），并把同一个令牌放进 meta 标签。运行时发出的每次调用都会在 `x-zeyn-csrf` 请求头中携带它，服务器以常量时间比较两者。
- 没有会话而打开的公开 screen 是例外：它们不设置 cookie，也不携带令牌，因此可以被缓存。运行时在第一次调用之前通过 `GET /_zeyn/csrf` 获取令牌，每个 `POST` 仍然以同样的方式检查（参见[公开页面](public-pages.md)）。
- 内容安全策略（Content Security Policy）只允许来自应用自身来源的脚本、样式和图片。没有内联脚本；主题是样式表，绝不是内联样式。
- `App::csp` 可以为指定的主机放开策略，每次放开一条指令。该响应头在启动时构建一次。

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  只接受 `https://host`、`https://host:port` 和 `https://*.host`（通配符匹配子域名，不匹配主机本身）。`.media_blob()` 会为流媒体库把 `blob:` 加入 `media-src`。`App::check` 会拒绝 `*`、`http:`、`data:` 或 `https:` 这类裸协议、`'unsafe-inline'` 或 `'unsafe-eval'` 这类带引号的关键字、路径、用户名，以及包含空格、逗号或分号的值；每个问题都会说明应该怎么写。`script-src`、`object-src`、`base-uri` 和 `frame-ancestors` 不能修改。`Embed` 和 `Image` 读取同样的列表，因此浏览器会拦截的主机永远不会被渲染（参见[展示](display.md)）。
- 响应带有 `X-Content-Type-Options: nosniff`、`Referrer-Policy: same-origin` 和 `X-Frame-Options: DENY`。
- `App::secure_cookies(true)` 会给 cookie 加上 `Secure` 标记；在 HTTPS 之后部署时请开启。

## 转义

HTML 中的用户数据一律经过 `render::escape`。组件会转义它渲染的每个值，因此名为 `<script>` 的商品会显示为文本。

## 限制

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit` 适用于除静态资源和公开媒体源读取（`Feature::public_media`）之外的所有请求；`write_limit` 为修改数据的调用增加更严格的限制：表单提交、grid 编辑、行操作、批量操作和菜单操作、命令、导入、已保存的 grid 视图、停靠布局和主题、API 调用、MCP 调用以及助手。被拒绝的请求会收到 `429` 和 `Retry-After`。
- `X-Forwarded-For` 只信任来自 `trusted_proxies` 的请求。该请求头的每一行都会按顺序计入，取用的地址是最后一个不属于受信任代理的地址。
- 揭示（reveal）、island 和数据源调用同样按写入计算，导出和打印的文档也一样（`GET /_zeyn/grid/export`、`GET /_zeyn/pdf`）。
- 无论客户端发送什么，页面大小、行数、查询长度和上传大小都会在服务器上设置上限。

## 会话

`App::idle_timeout(d)` 会在一段时间没有输入后让用户退出页面，之前会先显示警告对话框。标记为 `reauth` 的 action 需要近期登录过（`App::reauth_within`）。

## 数据保护

字段掩码、打开敏感记录前的原因说明、字段级审计和读取审计，详见[审计、掩码与作业](enterprise.md)。

## API 与智能体

API 调用需要 `Authorization` 请求头；仅凭浏览器会话 cookie 永远无法调用 API 或 MCP 端点，因此它们不需要 CSRF 令牌。智能体（agent）获得的权限与它所代表的用户完全相同。参见 [API 与 OpenAPI](api.md) 和 [MCP 与 AI 智能体](mcp.md)。
