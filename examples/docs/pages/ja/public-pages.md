# 公開ページ

`public` と指定した画面は、サインインしたことのない訪問者を含め、誰でも開けます。ランディングページ、
価格表、ヘルプセンターなどです。Zeyn は、これらを低コストで配信でき、キャッシュや検索エンジンが
扱いやすい形で提供します。

## 訪問者には Cookie を設定しない

セッションのないリクエストで開かれた公開画面は、Cookie を設定しません。ページには空の CSRF
トークンが入るため、同じ言語とテーマの訪問者には、すべて同じバイト列が返されます。

ブラウザのランタイムは、トークンが必要な最初の呼び出し（フォーム、アクション、言語ピッカー）の直前に、
`GET /_zeyn/csrf` でトークンを要求します。この呼び出しは `zeyn_csrf` Cookie を設定し、キャッシュされる
ことはありません。すべての `POST` は他のページとまったく同じように検査されます。トークンが一致しなければ
`403` で拒否され、黙って再試行されることはありません。

サインイン済みのユーザーには、従来どおり、常にページ内に Cookie とトークンが与えられます。

## HTTP キャッシュ

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N` は、ブラウザと共有キャッシュがページを `N` 秒間（1 から 86400）保持できるようにします。
セッションのないリクエストでは、ページに次のヘッダーが付きます。

| ヘッダー | 値 |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | レンダリングした本文に対する強いタグ |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

`If-None-Match` が一致するリクエストには、本文なしで `304 Not Modified` が返ります。アプリ内
ナビゲーションで読み込まれる画面も、独自のタグ付きで同じようにキャッシュされます。

セッションのあるリクエストに公開キャッシュのヘッダーが付くことはありません。ページは
`Cache-Control: private, no-store` で送られます。`public` でない画面に `cache` を指定するとコンパイル
できません。

画面は、サーバーに届いたすべてのリクエストに対して、引き続き構築、レンダリングされます。タグは転送を
節約し、アプリの前段にあるキャッシュはリクエストそのものを節約します。キャッシュされる画面には、
訪問者に依存するもの、たとえば時刻、カウンター、ランダムな選択を含めないでください。

## プリフェッチ

`App::prefetch(Prefetch::Moderate)` は、クリックの前に次の画面を読み込みます。デフォルトではオフです。`nav.js` は、リンクの上に 100 ms ホバーした後、キーボードフォーカス時、または押されたときに、そのリンクの部分画面を取得し、10 秒間保持し、同時に実行する取得は最大二つで、ポインターが先に離れたときは取得を取り消します。プリフェッチされるのは、アプリのパスでルーティングされた画面へのリンクだけで、`/_zeyn/*`、サインイン、サインアウトのリンク、ダウンロード、`target` 付きのリンク、`#[screen(no_prefetch)]` が付いた画面（実行前にサーバーが拒否します）は対象外です。Save-Data、`prefers-reduced-data`、2g 接続、非表示のタブでは何もプリフェッチされません。スクリプトがない場合、Chromium には `Speculation-Rules` ヘッダーを通じて Speculation Rules（プリフェッチのみで、プリレンダーはしません）が渡されるため、コンテンツセキュリティポリシーは変わりません。サーバーはこれらのリクエストに印を付けます。`ctx.is_prefetch()` が true になるので、画面は閲覧数のカウントを省略できます。公開キャッシュのルールは通常どおり適用されます。

## 検索エンジンとリンクプレビュー

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

| メソッド | head のタグ |
|---|---|
| `.description(text)` | `<meta name="description">` と `og:description`。ユーザーの言語で出力されます |
| `.canonical(url)` | `<link rel="canonical">` と `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` または `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

すべての値はエスケープされます。JSON-LD は `<`、`>`、`&` をそれぞれ `<`、`>`、`&` として
書き出すため、`</script>` を含む値で要素を終了させることはできません。ブラウザがそれを実行することはなく、
Content Security Policy も他のページと同じ厳格さのままです。JSON-LD のアドレス系プロパティ（`url`、`@id`、`image`、`logo`、`item` など）にあるアプリのパスは、検索エンジンが求めるとおり、どの深さでも `App::site_url` を付けた絶対アドレスで書き出されます。

URL は `https` の URL か、`/` で始まるアプリのパスでなければなりません。それ以外（`http:`、`//host`、
`javascript:`、空白、引用符）は、ログに警告を出して除外されます。アプリのパスは、サイト URL とベースパスの
後ろに書き出されます。

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

`site_url` がない場合、head のタグはアプリのパスのままになります。ブラウザはそれを解決できますが、
一部のクローラーはできません。

アプリ内ナビゲーションは、ページを読み込まずにタイトルとこれらすべてのタグを更新します。

## サイトマップ

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml` は、アクションに 1,000 件ずつ URL を要求し（`part.offset`、`part.limit`）、XML を
ストリーミングするため、大きなサイトでもメモリに載ることはありません。50,000 件までは 1 つの `urlset`
です。それを超えると、`/sitemaps/1.xml`、`/sitemaps/2.xml` などのサイトマップインデックスになり、それぞれ
に 50,000 件の URL が入ります。アクションは `public` で、URL を安定した順序で返す必要があります。
サイトマップは絶対 URL を保持するため、アプリのパスには `App::site_url` が必要です。`https` の URL でも
アプリのパスでもないエントリは除外されます。公開ページだけを列挙してください。検索エンジンはそれ以外を
開けません。サイトマップを持てる Feature は、アプリごとに 1 つです。

## robots.txt と llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

どちらも、プレーンテキストとして全員に提供されます。
