# 源代码、许可证与安全

Zeyn 是开源软件。本页说明代码在哪里、采用什么许可证、如何测试，以及如何报告安全问题。

## 源代码

代码和 issue 都在 GitHub 上：[github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn)。各个 crate 发布在 [crates.io](https://crates.io/crates/zeyn)，API 参考位于 [docs.rs](https://docs.rs/zeyn)。

## 许可证

Zeyn 采用双许可证：[MIT 许可证](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT)或 [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE)，由你任选其一（`Cargo.toml` 中为 `MIT OR Apache-2.0`）。贡献也以这两种许可证接受。

## 质量

仓库中共有 767 个测试函数（crate 和示例中的 `#[test]` 与 `#[tokio::test]`，统计于 2026-11-06），此外文档注释中的示例也会作为 doctest 运行。

每个 pull request 以及每次推送到 `dev` 和 `main` 都会运行 CI：

- `cargo fmt --all --check` 检查格式；
- `cargo clippy --workspace --all-targets -- -D warnings`，任何 lint 都会使构建失败；
- `cargo test --workspace`，运行每个 crate 和示例的测试；
- 使用 `assistant`、`oidc`、`totp`、`islands`、`realtime`、`xlsx`、`email` 和 `accounts` 特性运行 `cargo test -p zeyn`；
- `cargo test -p zeyn-cli`，它还会在不启用服务器特性的情况下构建框架；
- `cargo deny` 检查许可证、安全公告和被禁止的依赖。

每次运行的结果都公开在 [Actions 页面](https://github.com/huseyininnc/zeyn/actions)上。

## 安全

- **支持的版本。** Zeyn 处于 alpha 阶段，只有最新的 `0.0.x` 版本会获得安全修复。
- **报告方式。** 请通过 GitHub Security Advisories 私下报告漏洞：打开仓库的 Security 标签页并选择 "Report a vulnerability"（[直接链接](https://github.com/huseyininnc/zeyn/security/advisories/new)）。切勿在公开的 issue、讨论或 pull request 中报告。
- **响应。** 报告会在 5 个工作日内得到确认。修复发布后会公布安全公告，并致谢报告者，除非对方希望不署名。

完整的政策（包括报告应包含的内容和适用范围）见 [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md)。

## 更多

- [更新日志](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md)：每个版本的变更。
- [路线图](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md)：下一个版本和计划中的内容。
- [贡献指南](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md)：如何构建、测试和提交修改。
- [行为准则](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md)：我们如何一起协作。
- [支持](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md)：在哪里提问和报告 bug。
