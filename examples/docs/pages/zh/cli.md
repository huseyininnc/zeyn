# CLI 与项目

`zeyn` 命令用于创建项目、添加 feature、检查应用，并为智能体运行 MCP 服务器。

## 命令

| 命令 | 作用 |
|---|---|
| `zeyn new <name>` | 创建带有标准 feature 结构的应用 |
| `zeyn new <name> --lib` | 创建由多个应用共享的 model、feature 和文本库 |
| `zeyn g feature <name>` | 添加 `src/features/<name>`，包含 `models`、`services`、`actions` 和 `screens` 文件夹以及一个可用的列表 screen |
| `zeyn g screen <feature> <name>` | 向 feature 添加 `screens/<name>.rs`，注册它，并把标题加入每个语言文件 |
| `zeyn routes` | 列出 feature、screen、路由、API 和访问权限 |
| `zeyn check` | 编译并报告结构问题和缺失的翻译 |
| `zeyn check --fix` | 改写 `src`、`tests` 和 `examples` 中已重命名的 Zeyn API，然后检查 |
| `zeyn check --json` | 以 JSON 输出报告，包含稳定的代码、原因和修复方法 |
| `zeyn mcp` | 在 stdio 上为 AI 智能体运行 MCP 服务器 |

从 crates.io 安装：

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

在本仓库内，通过 Cargo 运行 CLI：

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## 新应用

`zeyn new` 会写出一个能够构建、通过测试并遵循智能体所获规则的项目：

- `AGENTS.md` 和 `CLAUDE.md`：项目规则和给智能体的框架索引。
- `.mcp.json`：注册 `zeyn` MCP 服务器，使智能体可以调用 `zeyn_check` 和 `zeyn_docs`。
- `config/app.json`：默认值。环境变量 `APP_*` 会覆盖它们，例如用 `APP_SERVER__ADDR` 覆盖 `server.addr`。
- `locales/en.json` 和 `locales/tr.json`：用户看到的每一段文本。
- `src/features/`：每个 feature 一个文件夹，带有一个示例 `notes` feature。
- `rustfmt.toml`、`clippy.toml`、`deny.toml`、`rust-toolchain.toml` 以及 `Cargo.toml` 中的 `[lints]`：由 CI 强制执行的标准。
  `rustfmt.toml` 设置了 `chain_width = 40`，因此代码自上而下阅读：超过该宽度的调用链
  每行放一个调用，平均每行一到两个点。智能体和人审阅列表比审阅一长行更快，
  而且 diff 会显示改动的那一个调用。请保留这项设置。

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  而不是 `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`。
- `build.rs` 以及 `Cargo.toml` 中 `[package.metadata.zeyn]` 下的 `max_file_lines = 300`：当 `src/` 中某个文件的非空行更多时，`cargo build` 会停止；clippy `too_many_lines`（60，在 `clippy.toml` 中）对函数做同样的检查。参见 [Project structure](structure.md)。
- `Dockerfile` 和 `.dockerignore`：体积小的容器镜像。
- 适用于常见 CI 服务器的流水线，全部运行相同的检查（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`、`cargo deny check`），并拒绝与 `Cargo.toml` 不匹配的标签 `vX.Y.Z`：
  - 用于 GitHub Actions 的 `.github/workflows/ci.yml`，以及 `.github/dependabot.yml`（每周的 Cargo 和 action 更新）和拉取请求检查清单；
  - 用于 GitLab 的 `.gitlab-ci.yml`；
  - 用于 Jenkins 的 `Jenkinsfile`（Docker agent，用 `cargo llvm-cov` 统计覆盖率）。
- `sonar-project.properties`：SonarQube 分析（Clippy 问题以及来自 `target/lcov.info` 的 LCOV 覆盖率）。当设置了仓库变量 `SONAR_HOST_URL` 和密钥 `SONAR_TOKEN` 时，GitHub 工作流会运行它；当作业设置 `SONAR_ENABLED=true` 时，Jenkins 会运行它。
- `SECURITY.md` 和 `CHANGELOG.md`（Keep a Changelog 格式）。
- 当安装了 git（Windows、macOS 或 Linux）且该文件夹尚不在某个仓库内时，会创建一个位于 `main` 分支的本地 git 仓库。不会提交任何内容，因此第一次提交会带上你自己的姓名和邮箱。`--no-git` 会跳过这一步。

各个文件夹的说明参见[项目结构](structure.md)。

## 生成器

`zeyn g feature` 创建一个 feature，每个 model、service、action 和 screen 一个文件：

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

简写 `zeyn g f` 和 `zeyn g s` 的作用相同。新 feature 立即可用：一个 model、一个内存数据 service、一个搜索 action，以及带菜单项的列表 screen。不需要某个文件夹的 feature 会删除它（文件夹以及 feature 的 `mod.rs` 中对应的 `mod` 行）。`components/` 从不生成：当 screen 的某个部分被共享或 screen 文件变长时，手动添加它，每个文件一个返回 `Node` 的函数（`components/<name>.rs`）。screen 获得权限 `<feature>.<name>.view`，其标题写入 `locales/` 中的每个文件；英语以外的语言先得到英文文本，并提示需要翻译。这种布局不是强制的：如果 feature 使用单个 `screens.rs`，`zeyn g screen` 会先把它连同内容移动到 `screens/mod.rs`；如果 `screens/` 已被删除，则重新创建它。生成器从不覆盖文件，并拒绝不是 snake case 的名称。

## 检查应用

`zeyn check` 先运行编译器，再运行应用自己的检查（`App::check`）：重复的路由、被注册两次的 screen、screen 需要但没人注册的服务、无法使用的快捷键、低于 WCAG AA 对比度的主题令牌，以及某种语言中缺失的翻译键。出现任何问题时它都会以错误退出，因此 CI 可以运行它。

它还会输出警告，但警告从不让检查失败：没有任何 feature 注册的 `#[screen]` 或 `#[action]`，以及超过约 150 行且包含多个条目的单个 `screens.rs` 或 `actions.rs`。

当 Zeyn 版本重命名某个 API 时，`zeyn check --fix` 会改写旧名称，例如把 `Grid` 改为 `DataGrid`，把 `GridQuery` 改为 `DataQuery`，然后再次检查。

```sh
zeyn check --fix
```

每个问题都有一个稳定的代码：`Z01xx` 表示应用、功能、路由和服务，`Z02xx` 表示安全，`Z03xx` 表示公开页面，`Z04xx` 表示菜单、命令和快捷键，`Z05xx` 表示数据，`Z06xx` 表示页面检查，`W01xx` 表示警告。文本输出会为每个问题显示原因（`why`）、修复方法（`help`）、指向本文档的链接和代码。`zeyn check --json` 将同一份报告作为一个 JSON 文档输出，供代理和 CI 使用，当 `ok` 为 false 时以错误退出。

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

## Docker

生成的 `Dockerfile` 分两个阶段构建，并在 distroless 镜像上以没有 root 权限的用户运行。二进制文件用 `<app> health` 检查自身的健康状况，`HEALTHCHECK` 使用它。

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

生成的应用的镜像约为 43 MB，空闲的应用只占用几 MB 内存。

## MCP 服务器

`zeyn mcp` 通过 stdio 使用 MCP 协议。它的工具有 `zeyn_docs`（按主题提供框架参考）、`zeyn_describe`（当前目录中应用的 feature、screen、路由和问题）、`zeyn_check`（带 `fix: true` 时运行 `check --fix`）、`zeyn_test`、`zeyn_new_feature` 和 `zeyn_new_screen`。参见 [MCP 与 AI 智能体](mcp.md)。
