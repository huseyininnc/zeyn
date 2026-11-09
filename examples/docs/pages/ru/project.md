# Исходный код, лицензия и безопасность

Zeyn распространяется с открытым исходным кодом. На этой странице сказано, где лежит код, под какой лицензией он выпущен, как он тестируется и как сообщить о проблеме безопасности.

## Исходный код

Код и issues находятся на GitHub: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). Крейты опубликованы на [crates.io](https://crates.io/crates/zeyn), а справочник по API находится на [docs.rs](https://docs.rs/zeyn).

## Лицензия

Zeyn выпускается под двумя лицензиями на ваш выбор: [MIT](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) или [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE) (`MIT OR Apache-2.0` в `Cargo.toml`). Вклад в проект принимается под теми же двумя лицензиями.

## Качество

В репозитории 837 тестовых функций (`#[test]` и `#[tokio::test]` в крейтах и примерах, подсчитано 2026-11-09), а также примеры в doc-комментариях, которые запускаются как doctest.

Каждый pull request и каждый push в `dev` и `main` запускает CI:

- `cargo fmt --all --check` для форматирования;
- `cargo clippy --workspace --all-targets -- -D warnings`, так что любое предупреждение lint останавливает сборку;
- `cargo test --workspace`, тесты всех крейтов и примеров;
- `cargo test -p zeyn` с функциями `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email` и `accounts`;
- `cargo test -p zeyn-cli`, который также собирает фреймворк без серверной функции;
- `cargo deny` для лицензий, уведомлений о безопасности и запрещённых зависимостей.

Результаты каждого запуска открыты на [странице Actions](https://github.com/huseyininnc/zeyn/actions).

## Безопасность

- **Поддерживаемые версии.** Zeyn находится на стадии alpha. Исправления безопасности получает только последний выпуск `0.0.x`.
- **Как сообщить.** Сообщайте об уязвимости приватно через GitHub Security Advisories: откройте вкладку Security репозитория и выберите "Report a vulnerability" ([прямая ссылка](https://github.com/huseyininnc/zeyn/security/advisories/new)). Никогда не сообщайте о ней в публичном issue, обсуждении или pull request.
- **Ответ.** Получение сообщения подтверждается в течение 5 рабочих дней. После выпуска исправления публикуется уведомление о безопасности, а автор сообщения упоминается, если он не попросил не называть его.

Полная политика, включая то, что указать в сообщении, и область её действия, находится в [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md).

## Дополнительно

- [Журнал изменений](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): изменения каждого выпуска.
- [Дорожная карта](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): следующий выпуск и планы.
- [Участие в проекте](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): как собрать, протестировать и отправить изменение.
- [Кодекс поведения](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): как мы работаем вместе.
- [Поддержка](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): где задать вопрос и сообщить об ошибке.
