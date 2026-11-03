# 运维

在生产环境运行 Zeyn 应用：时区数据、签名许可证、多台服务器的共享状态、日志与链路追踪、PDF 文档和电子邮件。

## 时区

`TimeZone::named` 接受所有 IANA 时区（`Europe/Istanbul`、`America/New_York`、`Australia/Sydney`，大小写均可）以及固定偏移（`UTC+03:00`、`+05:30`）。

- 时区数据是通过 `jiff` 内置于二进制文件中的 IANA 时区数据库（约 200 KB）。所有平台（包括 Windows）的结果都一致，也不会读取系统的 zoneinfo。
- 历史变更也有建模：土耳其在 2016 年之前的夏令时及此后的永久 `+03`、美国 2007 年之前的规则，以及其他所有有记录的变更。
- 不存在的本地钟面时间（时钟拨快）会被拒绝，返回 `LocalTimeError::Skipped`。出现两次的时间（时钟拨回）取较早的那个时刻。
- 时区在其名称首次被使用时从内置数据中读取，并在进程生命周期内保留，每个占用几千字节。未知名称不会被保留，因此客户端无法使缓存无限增长。
- `TimeZone::names()` 是供外壳（shell）时区选择器使用的常用时区简短列表。用户的时区若不在列表中，会为该用户添加到选择器里。
- 新的时区规则随较新的 `jiff` 版本发布（`cargo update -p jiff-tzdb`）。当某个国家更改规则时，请重新构建并部署。

## 签名许可证

许可证文件使用只有供应商持有的 ed25519 密钥签名。应用内嵌公钥，因此客户可以读取许可证，但无法修改它，也无法制作新的许可证。通过 `signed-licenses` Cargo feature 启用。

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json` 包含许可证字段：

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- 文件为 JSON，许可证内容以文本形式放在 `payload` 中，签名放在 `signature` 中。被签名的内容是 `zeyn-license-v1\n` 后接 payload 文本，因此被签名的字节恰好就是存储的字节。
- 私钥只保留在签名用的机器上。`keygen` 绝不会覆盖已有的密钥文件，新项目的 `.gitignore` 中已包含 `*.key`。`sign` 从 `--key`、`ZEYN_LICENSE_KEY` 变量或 `zeyn-license.key` 读取密钥，并拒绝没有 `holder` 的许可证，或 `valid_until` 不是 `YYYY-MM-DD` 格式的许可证。
- 被修改的文件、用其他密钥签名的文件，或根本不是许可证的文件都会被拒绝：`License::from_signed` 返回 `LicenseError`（`BadSignature`、`InvalidPublicKey`、`Malformed`），服务器会记录原因，所有 `Licensed` guard 都会以 "Your license does not cover this." 拒绝访问。无法读取的文件会保留上一份许可证，与任何失败的许可证来源一样。
- guard 在每次请求时按用户当地的今天检查有效期，因此过期的许可证会在无需重启的情况下以 "Your license has expired." 锁定应用。续期后的文件会在下次刷新时读取。
- ed25519 无法区分被修改的文件和用其他密钥签名的文件；两者都表现为签名不匹配，提示信息也如此说明。
- `max_users` 会被携带并签名，但框架不强制执行；由应用自行决定什么算作一个用户。

## 多台服务器：Redis 中的共享状态

启用 `redis` Cargo feature 后，`RedisStore` 会保存负载均衡器后面每个实例都必须一致的状态：

```rust
let redis = RedisStore::new(config.require("redis.url")?, "billing").map_err(Error::Internal)?;
App::new("Billing")
    .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
    .rate_store(redis.clone())
    .datagrid_views(redis)
```

服务器为每个用户保存哪些状态，以及保存在哪里：

| 状态 | 位置 | 共享 |
|---|---|---|
| 速率限制计数器 | `RateStore` | `RedisStore` |
| 已保存的表格视图和停靠布局 | `DataGridViewStore` | `RedisStore` |
| 空闲超时 | 浏览器（应用的所有标签页） | 服务器上不保存 |
| 登录与重新认证 | 单点登录代理 | 服务器上不保存 |
| 敏感屏幕的原因 | `HttpOnly` cookie | 服务器上不保存 |
| 表单草稿 | `sessionStorage` | 服务器上不保存 |
| 许可证缓存 | 每个进程，从来源重新加载 | 每台服务器各自加载 |
| 后台作业 | 每个进程 | 不共享；见 `enterprise.md` 的作业一节 |

- 速率限制是在一个 Lua 脚本中实现的令牌桶，因此不同服务器上的并发请求绝不会超过限制。时间来自 Redis，而不是应用服务器。计数器在最后一次使用后经过一个窗口时间即过期。
- 视图是位于 `zeyn:<prefix>:view:<user length>:<user>:<grid>` 下的 JSON，永不过期。
- 连接在首次使用时建立，断开后会自行恢复。调用在 500 毫秒后超时。Redis 宕机时，速率限制会放行请求，视图既不加载也不保存（表格回退到自身的布局，停靠面板回退到 `localStorage`），故障和恢复各记录一次日志，并且每次失败后五秒内不再访问 Redis，这样请求就不必各自等待超时。
- `RateStore::take` 返回 future，因此存储可以通过网络通信。`MemoryRateStore::take` 保持同步，便于直接使用。
- 未内置 TLS（`rediss://`）。
- 设置 `ZEYN_TEST_REDIS` 后，测试会针对真实服务器运行：

  ```
  docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
  ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
  docker stop zeyn-redis
  ```

## 日志与链路追踪

框架通过 `tracing` 输出信息，自身从不打印。

- `App::serve` 和 `App::serve_until` 会安装一个 subscriber，除非应用已先安装了自己的：事件以文本行输出到标准错误，并按 `log.level` 过滤（默认 `info`；支持任何 `tracing` target 过滤器，例如 `warn,zeyn=debug`）。需要 JSON 行或其他输出目标的应用，应在启动服务前安装自己的 subscriber，Zeyn 不会干涉。
- 在 `info` 级别，输出与以往一致：监听地址、启动警告（缺失的翻译、`DevAuth`、`dev_latency`）、失败的许可证来源和内部错误。在 `debug` 级别，每次调用还会增加一行 `call finished`，包含 kind、screen、action、用户 id、租户 id、结果、错误码和毫秒数。
- 供链路追踪后端使用的 span：

  | Span | 级别 | 字段 |
  |---|---|---|
  | `request` | info | method、path（不含查询字符串）、status、ms，以及已知后补充的：kind、screen、action、user、tenant、ok、error |
  | `run` | info | 应用代码：一个 action、一个数据源（表格取数、行查找、视图存储）、一个 screen 构建器 |
  | `interceptor` | debug | name（类型名，或 `Interceptor::name`）、stage（`before` 或 `after`） |

- 启用 `otel` Cargo feature 后，若在 `Config` 中设置了 `otel.endpoint`（`http://collector:4318`；span 发送到 `/v1/traces`），span 会通过 OTLP/HTTP 导出。`otel.service_name` 为服务命名，默认是应用名称。span 在后台线程中批量处理，并在 `serve_until` 停止时刷新。
- 不导出时，标准错误只接收事件，因此不会构建 span，每个 span 只花费一次缓存检查的开销。通过 `App::router` 挂载在其他服务器中的应用，以及测试，开销相同。
- 绝不记录的内容：字段值（无论是否被掩码）、表单和 API 输入、查询字符串、cookie、请求头和密钥。有一项测试会在 trace 级别的 subscriber 下运行表格、表单保存、API 调用和揭示（reveal）操作，并检查没有任何被掩码的值出现。有两件事仍由应用负责：路径会被记录，所以不要把个人数据放进路由；`Error::Internal` 的消息会按原样记录。

## PDF 文档

`pdf` Cargo feature 使用 [Typst](https://typst.app) 模板渲染 PDF 文档。由于 Typst 体积较大（见下方数据），该功能默认关闭。

```rust
const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("../templates/invoice.typ"));

#[action(permission = "sales.order.read")]
pub async fn invoice_data(p: OrderParams, store: Orders, ctx: Ctx) -> Result<InvoiceData> { ... }

Screen::window("Order").body(PdfLink::new("Print invoice", INVOICE, invoice_data, params))

App::new("Shop").service(Pdf::from_config(&config)?)
```

- `PdfLink` 在屏幕上放置一个下载按钮。`GET /_zeyn/pdf` 会连同其访问检查一起打开该屏幕，以该用户身份运行 action（其权限和拦截器都会生效，调用类型为 `Document`），并用 action 返回的内容渲染模板。无权运行该 action 的用户看不到此按钮。
- 带有 `exportable()` 的表格也可以通过工具栏的 `Export::Pdf`（`GET /_zeyn/grid/export?format=pdf`）打印列表报表：包含用户的列、标签、筛选和排序，数字右对齐，表头行重复显示，并带有页码。一份报表最多包含 2,000 行（`MAX_PDF_ROWS`，或表格的导出上限，取较小者），若有行被省略会给出提示。
- **数据绝不会变成 Typst 代码。** action 的输出被序列化为 JSON，模板将其作为 `data` 读取（渲染器会在模板之前绑定 `#let data = json("/data.json")`）。诸如 `#panic()`、`]` 或 `$x$` 之类的值会作为文本输出；有测试对此进行检查。模板无法读取其他任何内容：没有文件，没有包。
- 渲染在阻塞线程上运行。同时渲染的文档最多为 `pdf.max_concurrent` 个（1 到 64；默认两个，单 CPU 上为一个）；其余的等待空闲名额。Typst 的记忆化缓存在每份文档之后清空，因此内存会回落。

| 键 | 含义 |
|---|---|
| `pdf.fonts` | 额外的 `.ttf`、`.otf`、`.ttc` 或 `.otc` 字体所在目录，启动时读取一次 |
| `pdf.max_concurrent` | 同时渲染的文档数量 |

字体：内置了 Libertinus Serif（常规、粗体、斜体、粗斜体；SIL Open Font License），它涵盖土耳其语；还内置了 DejaVu Sans Mono 作为 Libertinus 缺少的符号（如 `₺`）的回退字体。它们的许可证位于 `crates/zeyn/assets/fonts`。没有数学字体；如果模板中使用公式，请通过 `pdf.fonts` 添加。

在 Windows 上的测量结果（release 构建，x86_64）：

| | 结果 |
|---|---|
| 二进制大小 | 不含 `pdf` 为 5.5 MB，含 `pdf` 约 46 MB（Typst 增加约 39 MB，字体 1.6 MB） |
| 一页文档 | 4 到 7 毫秒，渲染时堆内存不到 1 MB，文件 10 KB |
| 1,000 行的列表报表（25 页） | 0.54 到 0.74 秒，渲染时峰值堆内存 51 MB，之后保留 0，文件 0.87 MB |
| 启动后的第一份文档 | 约 42 MB 堆内存保留给 Typst 自己延迟构建的表 |
| 字体 | 字体索引占 0.25 MB 堆内存 |

debug 构建大约慢十倍（在 showcase 中，一份 2,000 行的报表耗时 71 秒）。

## 电子邮件

`email` Cargo feature 使用 [lettre](https://lettre.rs) 发送通知。

```rust
let mailer = Mailer::from_config(&config)?;
App::new("People").mailer(mailer)

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(r: TransitionRequest<LeaveRequestId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let state = leave_flow().apply(&ctx, current, &r.transition, r.comment.as_deref())?;
    mailer.send(&ctx, Mail::transition(&ctx, &manager_email, &employee)?)?;
    Ok(state)
}
```

- `Mailer::send` 渲染邮件并将其放入内存队列（最多 10,000 封），然后返回。`zeyn.mail` 后台作业会立即发送队列中的邮件，对失败的邮件按退避策略重试（除非 `Mailer::retry` 另有设置，默认从十秒开始共五次），并每分钟检查一次队列。邮件在失败八次后会被丢弃，并在日志中记录错误。与所有作业一样，进程停止时尚未发送的队列会丢失；若需要持久投递，请使用数据层中由数据库支持的队列。
- 邮件内容来自 i18n 目录：`<template>.subject`、`<template>.text` 以及可选的 `<template>.html`，带有 `{name}` 参数。收件人的语言与发送者相同，除非 `Mail::locale` 另行设置。每封邮件都有纯文本和 HTML 两部分；HTML 部分中每个参数都会被转义，没有 `.html` 条目时，HTML 即为转义后的文本。主题中的换行会被移除，地址会被解析，因此数据无法添加邮件头或收件人。
- `Mail::transition` 用内置的 `zeyn.mail.transition` 模板写出刚刚由 `Workflow::apply` 完成的状态迁移（记录、from、to、操作人和备注）。showcase 会在请假申请状态变化时通知经理。

| 键 | 含义 |
|---|---|
| `mail.from` | 发件人，例如 `Northwind <no-reply@example.com>`（必填） |
| `mail.transport` | `smtp`、`file`（每封邮件一个 `.eml` 文件，用于开发和测试）或 `log`（一行日志，包含收件人数量和大小，绝不包含地址或内容；默认值） |
| `mail.smtp.host`, `mail.smtp.port` | SMTP 服务器 |
| `mail.smtp.tls` | `starttls`（默认）、`tls`（隐式 TLS，通常为 465 端口）或 `none`（仅限本地中继） |
| `mail.smtp.user`, `mail.smtp.password` | 凭据；请从环境变量等密钥来源读取密码 |
| `mail.file.dir` | `file` 传输方式所用的目录 |

TLS 使用 rustls 和 Mozilla 根证书（`webpki-roots`）。
