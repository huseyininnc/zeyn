# 企业级功能

面向业务应用的通用功能：数据审计、个人数据保护、安全部署、健康检查、会话安全、CSV 与 XLSX 导入、后台任务和审批工作流。所有检查都在服务器端完成，浏览器运行时只负责让使用更顺手。设计时考虑了 KVKK 和 GDPR 这两部隐私法规，但这里的内容并不针对任何国家或行业。

## 字段级数据审计

`AuditLog` 会为每次表单提交以及每次对标记了 `audit` 的 action 的调用写入一条 `AuditEntry`。对于保存操作，该条目还会写明记录以及每个发生变化的字段：

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- 表单会自动完成这些工作：旧值来自屏幕在服务器上加载的记录，新值来自经过验证的 `Valid<M>`。客户端发送的任何内容都不会被用作旧值，表单不从客户端读取的字段也绝不会显示为已更改。
- API 保存会从其 `Valid<M>` 输入中记录 model 和主键。只有 action 知道数据库里有什么，所以由它自己报告差异：

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  在表单保存 action 中也可以这样做：action 的报告会替换表单的报告。
- 值是与语言无关的纯字符串（`1234.56 TRY`、`2024-05-01`、choice 的值）。行版本不记录。被掩码的字段保持掩码状态（见下文）。
- 保存失败时仍会写明记录，但没有变更内容。

## 字段掩码

用 `#[mask]` 标记存放个人数据的文本字段。下例中，身份证号显示为 `•••• 8901`，电子邮件显示为 `p••••@example.com`，备注显示为 `••••`：

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- 掩码在服务器端进行，覆盖网格单元格、表单、API 输出、agent 工具，因此也覆盖由单元格生成的 CSV 导出。没有权限时，原始值绝不会到达浏览器或 API 客户端。
- 取消掩码的权限是蛇形命名的 `<model>.<field>.unmask`（`person.national_id.unmask`），除非属性中指定了权限。
- 拥有权限的用户起初看到的仍是掩码文本，旁边有一个显示按钮。点击显示会调用 `POST /_zeyn/reveal`，它会再次检查权限并写入一条审计记录（kind 为 `reveal`，字段记在 `revealed` 中）。被拒绝的尝试同样会被审计。
- 拥有权限的 API 客户端可以获得原始值；每个这样的字段都会列在该调用审计记录的 `revealed` 中。
- 表单将被掩码的字段显示为只读文本。保存时会保留已存储的值，除非用户有权显示该字段，并且在显示之后修改了它。空的掩码字段是普通输入框，因此数据录入人员可以一次性填写，之后却无法读回。
- 对无权显示某被掩码字段的用户，针对该字段的过滤和排序会被忽略，这样就无法通过搜索来猜出值。这适用于对网格的每一次读取（分页、导出、对所有行的批量操作、已保存的视图、agent 工具）以及接受 `DataQuery` 的 API action。不要在被掩码的列上添加 `.filter()`。
- 分页游标绝不包含被掩码字段的值。按被掩码字段排序的列表按位置而不是按主键分页。
- 网格绝不按被掩码字段分组；对其调用 `group_by` 会被忽略并记录日志。
- Island 以及其他数据源的输出在任意深度都会被掩码，与 API 输出一样：其中每个 model 的被掩码字段都会以掩码文本输出，敏感 model 的主键会被记录为读取。
- 网格揭示原值时，会通过网格的 action 以主键过滤条件来查找该行，因此该 action 必须支持对其主键字段的过滤。
- `#[mask]` 只能用于 `String` 和 `Option<String>`，否则无法编译。

## 敏感记录读取

除了记录谁修改了数据之外，也会记录谁在何时看到了哪些个人数据。

- `#[entity(table = "cases", sensitive)]`（或者在 `#[entity]` 下方写 `#[sensitive]`）用于标记一个 model。每当向用户展示这样的记录时，该调用的审计记录就会在 `reads` 中列出它（`{"model":"Case","id":"2"}`）：包括网格页中的行、表单、agent 工具结果和 API 输出。读取记录在数据被渲染的地方写入，因此布局内部的组件同样会被计入。
- `#[screen(..., sensitive)]` 会把每次打开该屏幕记录为一次读取，其 `model` 为屏幕名称，`id` 为其 params，适用于展示不与某个 model 绑定的个人数据的屏幕。
- `#[screen(..., reason)]` 还会在展示任何数据之前询问用户原因。在给出原因之前，服务器会对该页面、其片段、网格、表单和揭示原值的调用返回 `428` 以及代码 `reason_required`（整页加载时返回带原因表单的页面）。运行时会弹出对话框询问，发送 `POST /_zeyn/reason {screen, reason}`（3 到 300 个字符），然后重复原请求。原因会保存在一个 `HttpOnly`、`SameSite=Strict` 的 cookie 中 15 分钟，只对给出原因的那个屏幕有效，并写入该屏幕每条审计记录的 `reason` 中。Agent 和 API 客户端无法给出原因，因此无法打开这类屏幕。
- 对敏感 model 的网格进行导出（CSV、XLSX、PDF）时，每个文件写入一条审计记录，并设置 `export`：包含 model、格式、行数，以及所请求的排序和过滤条件。它们不会在 `reads` 中列出每一行，所以大型导出仍然开销很小；其他 model 的导出不产生额外开销。从 `sensitive` 或 `reason` 屏幕导出时，还会记录对该屏幕的读取。
- 屏幕中的惰性部分（`POST /_zeyn/node`，由惰性标签页使用）会运行拦截器并记录读取，与打开屏幕完全一样。
- `ctx.record_read(&record)` 可在应用代码中记录一次读取。
- 读取记录由 `AuditLog` 为任何调用写入（包括页面加载），只要展示过敏感记录。没有审计拦截器就不会记录任何内容；持有个人数据的应用应当始终注册一个。

## 审批工作流

工作流是一个小型的、带类型的状态机。状态是一个 `#[choice]` 枚举；每次迁移都要指明起始状态、结束状态和所需的权限：

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply` 在服务器上运行，状态从存储中读取。未知的迁移返回 `404`，记录已不在起始状态返回 `409`（有人先一步移动了它），缺少权限返回 `403`，评论超过 1000 个字符返回 `422`。
- 每次迁移都会写入审计记录的 `transition`：名称、from、to 和评论；用户和时间在记录本身中。
- `Transitions::of(flow, state, id, action)` 显示当前状态、一个可选的评论，以及用户可以从当前状态发起的每个迁移对应的一个按钮（`Workflow::allowed`）。记录主键来自服务器上的屏幕，因此客户端无法移动其他记录。Agent 将其视为一个表单工具。
- 按钮标签来自目录中的 `workflow.<name>`；`.label(...)` 可手动设置一个。
- `Workflow::check` 会报告无效的权限，以及从同一状态声明了两次的迁移；请在测试中调用它。
- showcase 中的请假申请就使用了它：保存为草稿、提交，然后批准或拒绝。

## 安全部署

- 每个响应都带有 `x-zeyn-build`，每个页面都带有 `<meta name="zeyn-build">`。设置了 `ZEYN_BUILD_ID` 时，标识即为它的值（字母、数字、`.`、`_`、`-`，最多 64 个字符），否则是可执行文件的哈希。`zeyn::server::build_id()` 返回该标识，可用于日志。
- 当运行时看到来自另一个构建的响应时，会显示一条礼貌的提示：有新版本可用，准备好后请重新加载。它绝不会自行重新加载。
- 用户在表单中输入的内容会按屏幕、params 和表单保存在 `sessionStorage` 中，并在重新加载后恢复，同时给出提示和一个丢弃按钮。被掩码的、密码、文件和隐藏输入绝不会被保存。草稿会保留输入时的行版本，因此如果在此期间有人修改了该记录，保存草稿会因冲突而被拒绝。保存成功后草稿会被清除；超过一天的草稿会被丢弃。`sessionStorage` 只存在于该浏览器标签页中。

## 健康检查

- 进程运行期间，`GET /_zeyn/health` 返回 `200 ok`。用于存活探测。
- `GET /_zeyn/ready` 会同时运行通过 `App::readiness(name, check)` 添加的每个检查，每个限时五秒，并返回 `200 {"status":"ready"}` 或 `503 {"status":"not_ready","failed":["database"]}`。只显示检查名称，从不显示错误。
- 两者都位于 `base_path` 之下（`/billing/_zeyn/health`），响应带有 `Cache-Control: no-store`，且无需登录。
- 就绪检查的结果会复用一秒钟（`READY_CACHE`），并且同一时刻只有一个请求运行这些检查，所以大量探测请求不会给数据库造成负载。
- 健康检查从不受速率限制，因此繁忙的探测不会把自己锁在门外而导致健康的应用被重启。就绪检查会计入速率限制，除非它来自 `probe_ips` 中的地址。
- IP 规则仍然适用：`deny_ips` 始终生效，`allow_only_ips` 也生效，除非探测地址列在 `App::probe_ips([...])` 中，该设置只对它开放这两个端点。

## 开发延迟

`App::dev_latency(Duration::from_millis(400))` 会延迟每一次调用（页面、片段、网格、表单、API、MCP 和运行时调用；静态资源和健康探测除外），让开发者像处于慢速网络下的用户那样看到加载状态和重复点击。它只在 debug 构建中有效：在 release 构建中该方法什么也不做，延迟代码也会被编译掉，所以误留在代码中也不会拖慢生产环境。启用期间，启动时会输出一条警告。

## 会话安全

登录来自单点登录代理、内置的 OpenID Connect 登录或 bearer 令牌（见 `docs/auth.md`），因此这些功能只需要认证器告知用户是谁，对于重新认证，还需要告知其登录时间
（`Identity::authenticated_at`；`ForwardAuth::auth_time("x-auth-time")` 读取 Unix 秒数或 RFC 3339；`DevAuth` 将每个请求都报告为刚刚登录）。

- `App::idle_timeout(Duration)` 会让运行时在结束前 `idle_warning`（默认一分钟）通过一个带倒计时和“保持登录”按钮的警告对话框提醒用户，该按钮调用 `POST /_zeyn/keepalive`，以便前面的代理看到有活动。应用在任何标签页中的输入都会计入。到时间后，用户会被带到 `logout_url`，否则带到带有 `return_to` 的 `login_url`，否则看到一个已退出的屏幕。表单草稿会保留。空闲超时只用于关闭被遗忘的屏幕；结束真正的会话是身份提供方的工作。
- `#[action(permission = "...", reauth)]` 要求在 `App::reauth_within`（默认五分钟）之内登录过。否则无论客户端发送什么，服务器都会返回 `401` 以及代码 `reauth_required`（在 API 上对应的 problem 类型为 `urn:zeyn:problem:reauth_required`）。运行时会请用户在新标签页中重新登录（`reauth_url`，否则 `login_url`），并在用户返回后重复该请求，因此已输入的内容不会丢失。没有登录时间的身份永远无法通过。

## CSV 导入

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- 用户选择一个 CSV 文件。运行时将其按 UTF-8 读取，如果不是有效的 UTF-8，则按 Windows-1254 读取（土耳其语电子表格），并把文本放在 JSON 请求体中发送到 `POST /_zeyn/import`（无需 multipart；最大 8 MB）。服务器返回摘要、前十行的预览、每行的错误，以及可下载的错误报告。
- 解析：丢弃字节顺序标记，根据表头行检测 `,`、`;` 或制表符，支持 `"` 引号（引号内可含分隔符和换行），支持 `\r\n` 或 `\n`。
- 限制在读取文件时检查，而不是事后检查：读到行数上限即停止，超过 512 个单元格的表头会被拒绝，数据行最多比表头多保留一个单元格（因此多余的值仍会被报告）。XLSX 文件最多读取与表头列数相同的列，其表头最多只能指明 model 可导入的列数，超过 16 MB 的共享字符串部分会在解包任何内容之前被拒绝。
- 列按字段名或应用中任意语言的标签匹配，忽略大小写以及土耳其语带点和不带点的 i。未知列、只读列、主键列和行版本列会被拒绝。
- 每一行的读取和验证都与该 model 的表单完全一致：值按该语言用户的输入习惯解析（`1.234,56`、`05.12.2026`），应用字段规则和 model 自己的 `validate`。
- 导入只把有效行发送给 action；`.all_or_nothing()` 则在每一行都有效之前不导入任何内容。导入时，服务器会重新读取并验证文件；预览中的任何内容都不被信任。这两个步骤都需要该 action 的权限，并且导入会被审计。
- `.max_rows(n)` 限制行数（默认 5000，最大不超过 50000）。错误报告会中和电子表格公式。被掩码的字段在预览中以掩码形式显示。

### XLSX 导入

启用 `xlsx` Cargo feature 后，同一个组件也可以接受 Excel 工作簿（`.xlsx`）。运行时以 base64 发送文件（在同一个 JSON 请求体中用 `"xlsx"` 代替 `"csv"`），服务器使用 `calamine` 读取第一个工作表，并进入与 CSV 相同的逐行验证、预览和错误报告流程。

- 单元格保留其类型：数字被读作数字，日期单元格被读作日期，与用户的语言无关，因此在土耳其语会话中 `1234.5` 仍然是 1.234,50。日期单元格中的挂钟时间按用户的时区读取，就像表单那样。choice 按值或按标签匹配。
- 限制：文件最大 8 MB，压缩包解压后最多 64 MB、最多 1000 个部分，这些检查基于压缩包声明的大小，在解包任何内容之前进行。单元格逐个读取，读到行数上限即停止，因此一百万行的工作表的开销不会超过该上限。不是工作簿、受密码保护或被截断的文件会得到明确的错误。
- 只读取第一个工作表。公式按其缓存的值计算。

## 后台任务

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- 任务是 `async fn(Ctx) -> Result<()>`。它的上下文包含应用的服务、配置和默认语言，以及身份 `job:<name>`，该身份没有任何权限。
- `Schedule::every(d)` 按间隔运行；`Schedule::manual()` 只在被触发时运行。Action 通过可注入的 `Jobs` 服务触发任务：`jobs.trigger("rebuild_index")`。任务在运行期间被触发，则会在之后再运行一次；同一个任务绝不会同时运行两次。
- `.retry(times, first_delay)` 会在运行失败后重试，等待时间每次加倍，最长十分钟。
- 同时最多运行 `job_concurrency` 个任务（默认四个）。
- 状态（状态、运行次数、失败次数、上次开始和结束时间、上次错误代码）在代码中是 `jobs.status()`，对拥有 `jobs_permission` 的用户则是 `GET /_zeyn/jobs`。错误详情只会写入服务器日志。
- `App::serve_until(addr, signal)` 在信号完成时停止接收请求，让正在运行的请求完成，取消重试等待，并给正在运行的任务 30 秒。
- 没有存储时，任务保存在内存中：进程停止时，正在等待或正在运行的工作会丢失，而且有多台服务器时，每台都运行自己的调度。

### 持久化任务

```rust
App::new("Billing")
    .job_store(SqlJobStore::new(db))
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
jobs.enqueue("send_invoice", "42").await?;
```

- `JobStore` 保存运行记录：`enqueue`、`schedule`、带租约（工作进程 id 和结束时间）的 `claim`、`heartbeat`、`complete` 和 `fail`。`MemoryJobStore`（最多 10,000 条运行记录，最先丢弃最旧的失败记录）是默认实现；`SqlJobStore`（feature `postgres`、`oracle`）把运行记录保存在 `zeyn_jobs` 中。`create_job_tables(dialect, prefix)` 提供用于迁移的语句。
- 设置了存储后，`Schedule::every` 任务会把下一次运行时间保存在存储中，因此重启不会改变它，并且每个间隔只有一台服务器运行。`trigger` 也会在那里排入一次运行。
- `queued_job` 任务会获得每次运行的负载（最多 `MAX_JOB_PAYLOAD`，4000 字节）；普通任务也可以入队，但会忽略它。`enqueue_after` 可延迟一次运行。
- 认领（claim）会取走本服务器所知任务中到期的运行，最旧的优先，并持有 `JOB_LEASE`（60 秒），在运行期间每 20 秒续期一次。只有当工作进程仍持有租约时，`complete` 和 `fail` 才会更改一次运行。工作进程已停止的运行会在其租约结束后被再次认领；丢失的那次尝试也计入次数，因此让其服务器崩溃的运行最终会以失败结束。
- 失败的尝试会等待退避时间（每次加倍，最长 `MAX_BACKOFF_MS`，十分钟），只要还有剩余次数，然后以 `failed` 状态及其错误代码保留。计划任务则前进到下一个间隔。
- 工作进程每隔 `JOB_POLL`（一秒）查找到期的运行，并在同一服务器上 `enqueue` 之后立即查找。每次认领最多取走与空闲任务槽数量相同的运行，因此内存不会随队列增长。
- PostgreSQL 在一条 `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING` 中完成认领。Oracle 11g 及更高版本在事务中使用 `ROWNUM` 和 `FOR UPDATE SKIP LOCKED` 选取 id，更新它们，并在提交之前读回。Oracle 无法对加锁的 select 排序，因此那里到期的运行没有固定的取用顺序。时间是 Unix 毫秒，存放在 `BIGINT` 或 `NUMBER(19)` 列中。
- 设置了存储时，同一个任务的计划运行和被触发的运行可能重叠；不得同时运行两次的任务应自行加以保护（例如使用行锁）。
