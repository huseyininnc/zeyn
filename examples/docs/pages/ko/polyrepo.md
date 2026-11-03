# 별도 저장소의 앱과 공유 라이브러리

Zeyn은 단일 저장소에서 동작하지만, 많은 팀이 각 앱을 자체 저장소에 두고 공유 코드(entity, 서비스, Feature 전체)를 여러 앱이 사용하는 라이브러리에 둡니다. 이 가이드는 이를 설정하는 방법과, 이것이 동작하도록 유지하는 한 가지 규칙을 설명합니다.

## 무엇이 어디에 들어가는가

| 저장소 | 만드는 방법 | 내용 |
|---|---|---|
| 앱 | `zeyn new billing` | `main.rs`, `app.rs`, 앱 Feature, 설정, 앱 카탈로그 |
| 공유 라이브러리 | `zeyn new acme-domain --lib` | entity, 서비스, Feature, 그 카탈로그, `CATALOGS` |

라이브러리 Feature는 앱 Feature와 같습니다. 모델, action, Screen, 권한, 번역이 하나의 폴더에 함께 있습니다. 앱이 어떤 라이브러리 Feature를 마운트할지 결정합니다.

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

라이브러리 카탈로그가 먼저 로드되므로 앱은 같은 키를 정의하여 라이브러리 문자열을 덮어쓸 수 있습니다. 두 라이브러리가 충돌하지 않도록 라이브러리 키에는 Feature 이름을 접두사로 붙이십시오(`customers.list`).

`examples/shared-domain`이 그러한 라이브러리입니다. `examples/billing`과 `examples/crm`은 둘 다 그 customers Feature를 마운트하며, crm은 자체 카탈로그에서 "Customers"를 "Accounts"로 바꿉니다.

## 라이브러리가 소유해야 하는 것

Rust의 고아 규칙(orphan rule)에 따르면 트레이트는 그 트레이트나 타입을 정의한 크레이트에서만 구현할 수 있습니다. 앱은 라이브러리의 타입에 대해 `zeyn::Model`이나 `zeyn::Entity`를 구현할 수 없으므로 다음을 지키십시오.

- `#[entity]`, `#[model]`, `#[choice]`는 라이브러리의 타입에 그 규칙 옆에 붙이십시오.
- 검증 규칙(`#[rule(...)]`)은 공유 타입에 두십시오. 그러면 모든 앱이 같은 방식으로 검증합니다.
- 추가 동작이 필요한 앱은 타입을 감싸거나(`struct Invoice { customer: Customer, ... }`) 자체 트레이트를 추가합니다. 라이브러리 타입을 편집하지 않습니다.

## 앱이 라이브러리를 가져오는 방법

| 옵션 | 장점 | 주의할 점 |
|---|---|---|
| 태그가 있는 Git 의존성 | 실행할 것이 없고 모든 Git 서버에서 동작합니다 | Cargo가 저장소 전체를 가져오며, 모든 앱이 URL을 나열합니다 |
| 비공개 Cargo 레지스트리(Kellnr, GitLab 패키지 레지스트리 등) | 일반적인 `version = "0.3"` 의존성, semver 해석 | 실행하고, 백업하고, 보호해야 하는 서비스 |
| 모든 것을 담은 하나의 워크스페이스 저장소 | 하나의 빌드, 하나의 `Cargo.lock`, 한 번의 커밋으로 리팩터링 | 모든 팀이 모든 앱을 보며, 접근 제어는 폴더가 아니라 저장소 단위입니다 |

Git 태그로 시작하십시오. 라이브러리가 몇 개를 넘거나 버전을 손으로 해석하는 일이 번거로워지면 레지스트리로 옮기십시오. 앱 코드는 바뀌지 않고 `Cargo.toml`의 줄만 바뀝니다.

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new`는 기본적으로 `zeyn = "<version>"`을 씁니다. Zeyn이 공개되기 전까지는 모든 새 앱과 라이브러리에서 그 줄을 위의 Git 형태로 바꾸십시오.

## 규칙: 모든 곳에서 하나의 Zeyn 버전

모든 라이브러리와 모든 앱은 정확히 같은 Zeyn에 의존해야 합니다. Git 의존성에서는 같은 URL과 같은 태그를 뜻합니다.

Cargo는 서로 다른 두 Zeyn 소스를 서로 다른 두 크레이트로 취급합니다. 태그 `v0.0.2`로 빌드된 라이브러리가 반환하는 `zeyn::Feature`를 `v0.0.3`으로 빌드된 앱은 받아들일 수 없으며, 컴파일러는 이름이 같은 두 대상 사이의 타입 불일치를 보고합니다. 레지스트리에서는 이것이 "같은 semver 호환 버전"으로 완화되지만 규칙은 유지됩니다. 빌드당 하나의 Zeyn입니다.

Zeyn을 업그레이드하려면 다음과 같이 합니다.

1. 각 라이브러리의 태그를 업데이트하고, 테스트를 실행하고, 새 라이브러리 태그를 릴리스합니다.
2. 각 앱에서 Zeyn과 라이브러리 태그를 한 번의 커밋으로 함께 업데이트합니다.

`cargo tree -d -i zeyn`은 중복을 나열합니다. 둘 이상의 Zeyn이 출력되면 어떤 의존성이 아직 이전 태그를 가리키고 있는 것입니다.

## 단계별 안내

1. 라이브러리를 만들고 자체 저장소에 푸시합니다.

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. 태그와 함께 앱의 `Cargo.toml`에 추가하고, 위에 보인 대로 그 카탈로그, 서비스, Feature를 마운트합니다.

3. 앱의 CI가 라이브러리를 읽을 수 있게 허용합니다. 생성된 `.gitlab-ci.yml`은 `https://<gitlab host>/`와 `ssh://git@<gitlab host>/` URL을 작업 토큰을 사용하도록 다시 씁니다. GitLab에서 라이브러리 프로젝트를 열고, CI/CD 설정, Job token permissions로 이동하여 앱 프로젝트를 허용 목록에 추가하십시오.

4. 앱 테스트에서 `app.check()`를 실행합니다(생성된 `tests/app.rs`가 이를 합니다). 마운트된 Feature에 한 언어에 누락된 번역 키 같은 구조적 문제가 있으면 실패합니다.

## 라이브러리와 앱을 동시에 작업하기

`Cargo.toml`을 편집하지 않고 앱이 로컬 체크아웃을 가리키게 하십시오. 이를 앱의 `.cargo/config.toml`에 넣고 그 파일은 Git에서 제외하십시오.

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

변경이 끝나면 이를 제거하고, 라이브러리에 태그를 붙이고, 앱의 태그를 올리십시오.

## 라이브러리 릴리스

1. `Cargo.toml`의 `version`을 올립니다.
2. `cargo semver-checks`가 설치되어 있으면 실행합니다. 호출자를 깨뜨리는 공개 API 변경을 보고합니다. 라이브러리가 1.0 미만인 동안 호환이 깨지는 변경에는 새 마이너 버전이 필요합니다.
3. 커밋하고, `v<version>` 태그를 붙이고, 태그를 푸시합니다. 태그와 `Cargo.toml`이 일치하지 않으면 CI 작업 `tag-matches-version`이 실패합니다.
