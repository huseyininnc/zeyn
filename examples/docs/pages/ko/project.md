# 소스, 라이선스와 보안

Zeyn은 오픈 소스입니다. 이 페이지에서는 코드가 어디에 있는지, 어떤 라이선스로 배포되는지, 어떻게 테스트하는지, 보안 문제를 어떻게 신고하는지 설명합니다.

## 소스 코드

코드와 이슈는 GitHub에 있습니다: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). crate는 [crates.io](https://crates.io/crates/zeyn)에 게시되고 API 레퍼런스는 [docs.rs](https://docs.rs/zeyn)에 있습니다.

## 라이선스

Zeyn은 [MIT 라이선스](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) 또는 [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE) 중 원하는 것을 선택할 수 있는 이중 라이선스입니다(`Cargo.toml`의 `MIT OR Apache-2.0`). 기여도 같은 두 라이선스로 받습니다.

## 품질

저장소에는 953개의 테스트 함수가 있습니다(crate와 예제의 `#[test]`와 `#[tokio::test]`, 2026-11-10 기준). 여기에 더해 문서 주석의 예제가 doctest로 실행됩니다.

모든 pull request와 `dev`, `main`으로의 모든 push에서 CI가 실행됩니다:

- 서식 검사 `cargo fmt --all --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`, 어떤 lint 경고도 빌드를 실패시킵니다;
- `cargo test --workspace`, 모든 crate와 예제의 테스트;
- `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email`, `accounts` 기능을 켠 `cargo test -p zeyn`;
- 서버 기능 없이 프레임워크도 빌드하는 `cargo test -p zeyn-cli`;
- 라이선스, 보안 권고, 금지된 의존성을 검사하는 `cargo deny`.

모든 실행 결과는 [Actions 페이지](https://github.com/huseyininnc/zeyn/actions)에 공개되어 있습니다.

## 보안

- **지원 버전.** Zeyn은 alpha 단계입니다. 최신 `0.0.x` 릴리스만 보안 수정을 받습니다.
- **신고 방법.** 취약점은 GitHub Security Advisories로 비공개 신고하세요. 저장소의 Security 탭을 열고 "Report a vulnerability"를 선택합니다([바로 가기](https://github.com/huseyininnc/zeyn/security/advisories/new)). 공개 이슈, 토론, pull request에는 절대 올리지 마세요.
- **응답.** 신고는 영업일 기준 5일 안에 확인합니다. 수정이 릴리스되면 보안 권고를 게시하고, 신고자가 원하지 않는 경우를 빼고 신고자를 밝힙니다.

신고에 담을 내용과 적용 범위를 포함한 전체 정책은 [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md)에 있습니다.

## 더 보기

- [변경 기록](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): 릴리스마다의 변경 사항.
- [로드맵](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): 다음 릴리스와 계획.
- [기여 가이드](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): 빌드, 테스트, 변경 제출 방법.
- [행동 강령](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): 함께 일하는 방식.
- [지원](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): 질문하고 버그를 신고하는 곳.
