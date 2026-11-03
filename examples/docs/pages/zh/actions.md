# Action

action 是服务器端代码，界面（screen）、表格、表单、命令、JSON API 或智能体（agent）都可以调用它。`#[action]` 声明谁可以调用它；框架会在代码运行之前检查这一点。

## 声明 action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- 第一个参数是输入。它来自客户端，因此框架将其作为不受信任的 JSON 读取。
- 其他所有参数都是注入的：`Ctx` 或已注册的 `Service`。
- 返回类型是 `Result<T>`。

## 访问控制

每个 action 必须且只能声明以下其中之一：

| 选项 | 含义 |
|---|---|
| `permission = "feature.resource.verb"` | 身份需要具备此权限 |
| `public` | 任何人，包括尚未登录的用户 |

没有声明访问控制的 action 无法通过编译。权限名称为小写并以点分隔，例如 `sales.order.write`。

其他选项：

| 选项 | 含义 |
|---|---|
| `audit` | 每次调用都写入一条审计记录 |
| `reauth` | 用户必须是最近登录的（`App::reauth_within`） |

## 错误

| 错误 | 状态码 | 用途 |
|---|---|---|
| `Error::NotFound` | 404 | 记录不存在，或用户无权得知其存在 |
| `Error::Forbidden` | 403 | 拒绝访问 |
| `Error::Denied(t!("..."))` | 403 | 拒绝访问，并附带用户可以阅读的原因 |
| `Error::Conflict` | 409 | 自表单加载以来，行版本已发生变化 |
| `Error::Validation(errors)` | 422 | 字段错误，显示在字段旁边 |
| `Error::Internal("detail")` | 500 | 详细信息只写入服务器日志 |

在代码中产生字段错误：

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## 列表

列表接受 `DataQuery` 并返回 `Page<M>`。列表 action 绝不会返回所有行；表格按页请求，并且无论客户端发送什么，服务器都会限制页大小。

## 上下文

`Ctx` 代表请求：`ctx.identity()`、`ctx.locale()`、`ctx.time_zone()`、`ctx.now()`、`ctx.today()`、`ctx.tenant()`、`ctx.license()`、`ctx.config()`、`ctx.theme()` 和 `ctx.t(&text)`。请使用 `ctx.now()` 和 `ctx.today()` 而不是系统时钟，这样测试可以通过 `App::fixed_now` 固定时间。

## action 的使用位置

- `DataGrid::of(action)` 加载行；`.edit_with(action)`、`.row_action(...)` 和 `.bulk_action(...)` 修改行。
- `Form::submit(label, action)` 保存表单。
- `ActionButton::new(label, action, input)` 从按钮运行 action。
- `Command::action(...)` 从命令面板或快捷键运行 action。
- `Feature::api(action)` 将其发布为 JSON（参见 [API 与 OpenAPI](api.md)）。
- `Feature::search(action)` 为命令面板添加服务器端搜索。

每条路径都会运行相同的检查：浏览器调用的 CSRF 检查、Feature 的守卫（guard）、action 的访问控制，然后是拦截器（interceptor）。
