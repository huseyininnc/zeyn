# 应用与共享库分属不同仓库

Zeyn 可以在单个仓库中工作，但许多团队会让每个应用拥有自己的仓库，并把共享代码（实体、服务、完整的 feature）放进由多个应用共同使用的库中。本指南说明如何搭建这种结构，以及保证它正常运作的那条规则。

## 各放什么

| 仓库 | 创建方式 | 内容 |
|---|---|---|
| 应用 | `zeyn new billing` | `main.rs`、`app.rs`、应用 feature、配置、应用目录 |
| 共享库 | `zeyn new acme-domain --lib` | 实体、服务、feature、它们的目录、`CATALOGS` |

库中的 feature 与应用中的 feature 相同：model、action、screen、权限和翻译都放在同一个文件夹中。由应用决定挂载哪些库 feature。

```rust
let i18n = I18n::new("en")
    .catalogs(acme_domain::CATALOGS)?
    .catalog("en", include_str!("../locales/en.json"))?
    .catalog("tr", include_str!("../locales/tr.json"))?;

App::new(t!("app.name"))
    .i18n(i18n)
    .service(acme_domain::customers::CustomerStore::default())
    .feature(acme_domain::customers::feature())
```

库的目录会最先加载，因此应用可以通过定义相同的键来覆盖库中的字符串。请给库的键加上 feature 名称前缀（`customers.list`），这样两个库就不会冲突。

`examples/shared-domain` 就是这样一个库。`examples/billing` 和 `examples/crm` 都挂载了它的 customers feature；crm 在自己的目录中把“Customers”改名为“Accounts”。

## 库必须拥有什么

Rust 的孤儿规则规定，trait 只能在定义该 trait 或该类型的 crate 中实现。应用无法为来自库的类型实现 `zeyn::Model` 或 `zeyn::Entity`，因此：

- 把 `#[entity]`、`#[model]` 和 `#[choice]` 放在库中的类型上，紧挨着它的规则。
- 把验证规则（`#[rule(...)]`）保留在共享类型上。这样每个应用的验证方式都相同。
- 需要额外行为的应用，应当包装该类型（`struct Invoice { customer: Customer, ... }`）或添加自己的 trait。不要编辑库中的类型。

## 应用如何获得库

| 方式 | 优点 | 注意事项 |
|---|---|---|
| 带标签（tag）的 Git 依赖 | 无需运行任何服务，适用于任何 Git 服务器 | Cargo 会获取整个仓库；每个应用都要列出该 URL |
| 私有 Cargo 注册表（Kellnr、GitLab 包注册表等） | 常规的 `version = "0.3"` 依赖，semver 解析 | 需要运行、备份并保护一项服务 |
| 所有内容放在一个工作区仓库 | 一次构建、一个 `Cargo.lock`、重构在一次提交中完成 | 每个团队都能看到每个应用；访问控制以仓库为单位，而不是以文件夹为单位 |

先从 Git 标签开始。当库多于少数几个，或者手动处理版本解析变得繁琐时，再迁移到注册表。应用代码不会变化，只有 `Cargo.toml` 中的几行会变。

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new` 默认写入 `zeyn = "<version>"`。在 Zeyn 发布之前，请在每个新应用和库中把这一行改成上面的 Git 形式。

## 规则：各处使用同一个 Zeyn 版本

每个库和每个应用都必须依赖完全相同的 Zeyn。对于 Git 依赖，这意味着相同的 URL 和相同的标签。

Cargo 会把两个不同的 Zeyn 来源视为两个不同的 crate。针对标签 `v0.0.2` 构建的库返回的 `zeyn::Feature`，针对 `v0.0.3` 构建的应用无法接受，编译器会报告两个同名事物之间的类型不匹配。注册表把这一要求放宽为“同一个 semver 兼容版本”，但规则不变：一次构建只能有一个 Zeyn。

升级 Zeyn 的步骤：

1. 在每个库中更新标签，运行其测试，发布新的库标签。
2. 在每个应用中一起更新 Zeyn 和库的标签，放在同一次提交中。

`cargo tree -d -i zeyn` 会列出重复项。如果它输出了不止一个 Zeyn，说明某个依赖仍然指向旧标签。

## 分步操作

1. 创建库并把它推送到它自己的仓库。

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. 带着标签把它添加到应用的 `Cargo.toml` 中，并按上文所示挂载它的目录、服务和 feature。

3. 允许应用的 CI 读取该库。生成的 `.gitlab-ci.yml` 会把 `https://<gitlab host>/` 和 `ssh://git@<gitlab host>/` 的 URL 改写为使用作业令牌（job token）。在 GitLab 中，打开该库项目，进入 CI/CD 设置、Job token 权限，然后把应用项目添加到允许列表中。

4. 在应用测试中运行 `app.check()`（生成的 `tests/app.rs` 就是这样做的）。当某个已挂载的 feature 存在结构性问题时，例如某个翻译键缺少某种语言，它就会失败。

## 同时开发库和应用

无需编辑 `Cargo.toml`，就可以让应用指向你本地的检出。把下面的内容放进应用的 `.cargo/config.toml`，并且不要把该文件提交到 Git：

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

改动完成后，删除它，给库打标签，并在应用中提升标签。

## 发布库

1. 提升 `Cargo.toml` 中的 `version`。
2. 如果已安装，运行 `cargo semver-checks`。它会报告会使调用方出错的公共 API 变更。在库的版本低于 1.0 时，破坏性变更需要新的次版本号。
3. 提交，打标签 `v<version>`，推送标签。当标签与 `Cargo.toml` 不一致时，CI 作业 `tag-matches-version` 会失败。
