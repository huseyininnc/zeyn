# 공개 페이지

`public`으로 표시된 Screen은 로그인한 적 없는 방문자를 포함해 누구나 열 수 있습니다. 랜딩 페이지, 가격표, 도움말 센터가 그 예입니다. Zeyn은 이런 페이지를 전달 비용이 낮고 캐시와 검색 엔진이 처리하기 쉽게 제공합니다.

## 방문자에게는 쿠키 없음

세션이 없는 요청이 연 공개 Screen은 쿠키를 설정하지 않습니다. 페이지에는 빈 CSRF 토큰이 들어 있으므로 같은 언어와 테마의 모든 방문자가 같은 바이트를 받습니다.

브라우저 런타임은 토큰이 필요한 첫 호출(폼, action, 언어 선택기) 직전에 `GET /_zeyn/csrf`로 토큰을 요청합니다. 이 호출은 `zeyn_csrf` 쿠키를 설정하며 캐시되지 않습니다. 모든 `POST`는 다른 페이지와 똑같이 검사됩니다. 일치하는 토큰이 없으면 `403`으로 거부되며, 조용히 재시도하는 일은 없습니다.

로그인한 사용자는 이전과 같이 항상 쿠키와 페이지 안의 토큰을 받습니다.

## HTTP 캐싱

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N`은 브라우저와 공유 캐시가 페이지를 `N`초(1에서 86400) 동안 보관하도록 허용합니다. 세션이 없는 요청에는 페이지가 다음을 받습니다.

| 헤더 | 값 |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | 렌더링된 본문에 대한 강한 태그 |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

일치하는 `If-None-Match`가 있는 요청은 본문 없이 `304 Not Modified`를 받습니다. 앱 내 탐색으로 불러오는 Screen도 자체 태그와 함께 같은 방식으로 캐시됩니다.

세션이 있는 요청은 공개 캐시 헤더를 받지 않습니다. 페이지는 `Cache-Control: private, no-store`와 함께 전송됩니다. `public`이 아닌 Screen에 `cache`를 쓰면 컴파일되지 않습니다.

서버에 도달하는 모든 요청에 대해 Screen은 여전히 만들어지고 렌더링됩니다. 태그는 전송을 줄이고, 앱 앞단의 캐시는 요청을 줄입니다. 캐시되는 Screen에는 방문자에 따라 달라지는 것을 넣지 마십시오. 하루 중의 시각, 카운터, 무작위 선택이 그 예입니다.

## 프리페치

`App::prefetch(Prefetch::Moderate)`는 클릭하기 전에 다음 Screen을 불러오며, 기본적으로 꺼져 있습니다. `nav.js`는 링크 위에 100 ms 동안 호버하거나 키보드 포커스가 오거나 누를 때 그 링크의 부분 Screen을 가져와 10초 동안 보관하고, 동시에 최대 두 개까지만 가져오며, 포인터가 먼저 벗어나면 그 가져오기를 취소합니다. 앱의 경로가 있는 Screen으로 가는 링크만 프리페치하며, `/_zeyn/*`, 로그인 및 로그아웃 링크, 다운로드, `target`이 있는 링크, `#[screen(no_prefetch)]`로 표시된 Screen은 프리페치하지 않습니다. 서버는 이런 Screen이 실행되기 전에 요청을 거부합니다. Save-Data, `prefers-reduced-data`, 2g 연결, 숨겨진 탭에서는 아무것도 프리페치하지 않습니다. 스크립트가 없는 곳에서는 Chromium이 `Speculation-Rules` 헤더를 통해 Speculation Rules(프리페치만, 프리렌더는 없음)를 받으므로 콘텐츠 보안 정책은 그대로 유지됩니다. 서버는 이런 요청에 표시를 하므로 `ctx.is_prefetch()`가 true이고, Screen은 조회수 집계를 건너뛸 수 있습니다. 공개 캐시 규칙은 평소대로 적용됩니다.

## 검색 엔진과 링크 미리 보기

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new(t!("pricing.title"))
        .description(t!("pricing.description"))
        .canonical("/pricing")
        .og_image("https://cdn.example.com/pricing.png")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Team plan",
        }))
}
```

| 메서드 | head 태그 |
|---|---|
| `.description(text)` | 사용자의 언어로 된 `<meta name="description">`과 `og:description` |
| `.canonical(url)` | `<link rel="canonical">`과 `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` 또는 `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

모든 값은 이스케이프됩니다. JSON-LD는 `<`, `>`, `&`를 `<`, `>`, `&`으로 써서 기록하므로, `</script>`를 담은 값이 요소를 끝낼 수 없습니다. 브라우저는 이를 실행하지 않으며 콘텐츠 보안 정책은 다른 모든 페이지만큼 엄격하게 유지됩니다. JSON-LD의 주소 속성(`url`, `@id`, `image`, `logo`, `item` 등)에 있는 앱 경로는 깊이와 관계없이 검색 엔진이 기대하는 대로 `App::site_url`을 붙인 절대 주소로 기록됩니다.

URL은 `https` URL이거나 `/`로 시작하는 앱 경로여야 합니다. 그 밖의 것(`http:`, `//host`, `javascript:`, 공백, 따옴표)은 로그에 경고를 남기고 제외됩니다. 앱 경로는 사이트 URL과 기본 경로 뒤에 기록됩니다.

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

`site_url`이 없으면 head 태그에는 앱 경로가 그대로 남으며, 브라우저는 이를 해석하지만 일부 크롤러는 해석하지 못합니다.

앱 내 탐색은 페이지 로드 없이 제목과 이 모든 태그를 갱신합니다.

## 사이트맵

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml`은 action에 한 번에 1,000개의 URL(`part.offset`, `part.limit`)을 요청하고 XML을 스트리밍하므로 큰 사이트도 메모리에 올라오지 않습니다. URL이 50,000개까지는 하나의 `urlset`입니다. 그보다 많으면 `/sitemaps/1.xml`, `/sitemaps/2.xml` 등의 사이트맵 인덱스가 되며 각각 URL을 50,000개씩 담습니다. action은 `public`이어야 하고 URL을 안정적인 순서로 반환해야 합니다. 사이트맵은 절대 URL을 담으므로 앱 경로에는 `App::site_url`이 필요하며, `https` URL이나 앱 경로가 아닌 항목은 제외됩니다. 공개 페이지만 나열하십시오. 검색 엔진은 나머지를 열 수 없습니다. 앱당 하나의 Feature만 사이트맵을 가집니다.

## robots.txt와 llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

둘 다 모든 사람에게 일반 텍스트로 제공됩니다.
