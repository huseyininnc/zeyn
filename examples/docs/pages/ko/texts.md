# 텍스트와 t!

사용자에게 보이는 모든 텍스트는 `t!`를 통해 로케일 파일에서 가져옵니다. 컴파일러는 모든 로케일 파일에 키가 있는지, 인수가 플레이스홀더와 일치하는지 검사하므로, 번역이 누락되면 빈 레이블이 아니라 빌드 오류가 됩니다.

## 로케일 파일

`locales/<lang>.json`은 중첩된 키를 담습니다. 모든 파일의 키가 같아야 합니다.

```json
{
  "stock": {
    "products": "Products",
    "saved": "Product {code} saved."
  }
}
```

## t!

```rust
Screen::new(t!("stock.products"))
ui.toast(t!("stock.saved", code = p.code));
```

`t!`는 `Text`를 반환합니다. 키와 그 인수로 이루어지며 페이지가 렌더링될 때 사용자의 언어로 해석됩니다. 오타는 컴파일 시점에 실패합니다.

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

## 필드와 choice의 레이블

필드 레이블은 `Product.code`처럼 `<Model>.<field>` 키를 사용합니다. choice 레이블은 `<Choice>.<Variant>`를 사용합니다. 키가 없으면 레이블은 `#[label("...")]` 또는 이름을 읽기 좋게 바꾼 형태로 대체됩니다. `zeyn check`는 일부 언어에는 있고 다른 언어에는 없는 키를 보고합니다.

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## 언어 등록

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

`I18n::new`의 첫 번째 인수는 기본 언어입니다. 카탈로그가 둘 이상이면 셸에 언어 선택기가 표시됩니다. 프레임워크 자체의 텍스트(그리드, 폼, 셸)는 영어와 터키어로 제공됩니다.

## Screen 밖의 텍스트

- `ctx.t(&text)`는 이메일처럼 `Text`를 사용자의 언어로 된 `String`으로 바꿉니다.
- `Text::english()`는 로그와 테스트를 위한 영어 텍스트를 제공합니다.
- `Text::from("literal")`은 제품 코드처럼 번역되지 않는 텍스트입니다.

언어가 어떻게 선택되고 숫자, 금액, 날짜가 어떻게 서식화되는지는 [시간과 언어](time-and-languages.md)를 참고하십시오.
