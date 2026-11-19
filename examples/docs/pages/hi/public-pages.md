# सार्वजनिक पृष्ठ

`public` चिह्नित Screen कोई भी खोल सकता है, उन आगंतुकों समेत जिन्होंने कभी साइन इन नहीं किया: लैंडिंग पृष्ठ, मूल्य सूची, सहायता केंद्र। Zeyn उन्हें इस तरह परोसता है कि उन्हें पहुँचाना सस्ता हो और कैश तथा सर्च इंजन के लिए संभालना आसान।

## आगंतुकों के लिए कोई कुकी नहीं

बिना सत्र वाले अनुरोध से खुली सार्वजनिक Screen कोई कुकी सेट नहीं करती। पृष्ठ में खाली CSRF टोकन होता है, इसलिए एक ही भाषा और थीम वाले हर आगंतुक को वही बाइट मिलते हैं।

ब्राउज़र रनटाइम टोकन चाहिए वाली पहली कॉल (फ़ॉर्म, action, भाषा चयनकर्ता) से ठीक पहले `GET /_zeyn/csrf` से टोकन माँगता है। वह कॉल `zeyn_csrf` कुकी सेट करती है और कभी कैश नहीं होती। हर `POST` ठीक वैसे जाँचा जाता है जैसे किसी भी अन्य पृष्ठ पर: मेल खाते टोकन के बिना उसे `403` से अस्वीकार किया जाता है, और कुछ भी चुपचाप दोबारा नहीं आज़माया जाता।

साइन इन उपयोगकर्ताओं को पहले की तरह हमेशा पृष्ठ में कुकी और टोकन मिलते हैं।

## HTTP कैशिंग

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N` ब्राउज़र और साझा कैश को पृष्ठ `N` सेकंड (1 से 86400) तक रखने देता है। बिना सत्र वाले अनुरोध के लिए पृष्ठ को तब यह मिलता है:

| हेडर | मान |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | रेंडर की गई बॉडी पर एक मज़बूत टैग |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

मेल खाते `If-None-Match` वाले अनुरोध को बिना बॉडी के `304 Not Modified` मिलता है। ऐप के भीतर नेविगेशन से लोड होने वाली Screen उसी तरह कैश होती हैं, अपने टैग के साथ।

सत्र वाले अनुरोध को कभी सार्वजनिक कैश हेडर नहीं मिलते: पृष्ठ `Cache-Control: private, no-store` के साथ भेजा जाता है। जो Screen `public` नहीं है उस पर `cache` कंपाइल नहीं होता।

सर्वर तक पहुँचने वाले हर अनुरोध के लिए Screen फिर भी बनाई और रेंडर की जाती है; टैग ट्रांसफ़र बचाता है, और ऐप के सामने का कैश अनुरोध बचाता है। कैश की जाने वाली Screen को आगंतुक पर निर्भर हर चीज़ से मुक्त रखें: दिन का समय, काउंटर, यादृच्छिक चयन।

## Prefetch

`App::prefetch(Prefetch::Moderate)` क्लिक से पहले अगली स्क्रीन लोड कर लेता है; यह डिफ़ॉल्ट रूप से बंद है। `nav.js` लिंक की आंशिक स्क्रीन को 100 ms hover के बाद, कीबोर्ड फ़ोकस पर या दबाने पर लाता है, उसे 10 सेकंड रखता है, एक समय में अधिकतम दो ऐसे fetch चलाता है और पॉइंटर पहले हट जाए तो उसे रद्द कर देता है। केवल ऐप की पथ-रूट वाली स्क्रीन के लिंक prefetch होते हैं, `/_zeyn/*`, साइन इन और साइन आउट लिंक, डाउनलोड, `target` वाले लिंक या `#[screen(no_prefetch)]` से चिह्नित स्क्रीन कभी नहीं, जिन्हें सर्वर चलने से पहले ही अस्वीकार कर देता है। Save-Data, `prefers-reduced-data`, 2g कनेक्शन या छिपे टैब में कुछ भी prefetch नहीं होता। जहाँ स्क्रिप्ट नहीं है, वहाँ Chromium को `Speculation-Rules` हेडर के माध्यम से Speculation Rules मिलते हैं (केवल prefetch, कभी prerender नहीं), इसलिए content security policy जैसी है वैसी रहती है। सर्वर इन अनुरोधों को चिह्नित करता है: `ctx.is_prefetch()` true होता है, इसलिए कोई स्क्रीन दृश्य गिनना छोड़ सकती है। सार्वजनिक कैश नियम हमेशा की तरह लागू होते हैं।

## सर्च इंजन और लिंक पूर्वावलोकन

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

| मेथड | हेड टैग |
|---|---|
| `.description(text)` | उपयोगकर्ता की भाषा में `<meta name="description">` और `og:description` |
| `.canonical(url)` | `<link rel="canonical">` और `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` या `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

हर मान escape होता है। JSON-LD में `<`, `>` और `&` को `<`, `>` और `&` के रूप में लिखा जाता है, इसलिए `</script>` वाला मान एलिमेंट को खत्म नहीं कर सकता; ब्राउज़र उसे कभी नहीं चलाता, और content security policy उतनी ही सख़्त रहती है जितनी हर अन्य पृष्ठ पर। JSON-LD की पता प्रॉपर्टी (`url`, `@id`, `image`, `logo`, `item` आदि) में ऐप पाथ किसी भी गहराई पर `App::site_url` के साथ पूर्ण पते के रूप में लिखे जाते हैं, जैसा सर्च इंजन चाहते हैं।

URL `https` URL या `/` से शुरू होने वाले ऐप पथ होने चाहिए। कुछ भी और (`http:`, `//host`, `javascript:`, खाली जगह, उद्धरण) लॉग में चेतावनी के साथ छोड़ दिया जाता है। ऐप पथ साइट URL और बेस पथ के पीछे लिखे जाते हैं:

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

`site_url` के बिना हेड टैग ऐप पथ रखते हैं, जिसे ब्राउज़र हल कर लेते हैं पर कुछ क्रॉलर नहीं।

ऐप के भीतर नेविगेशन शीर्षक और इन सभी टैग को बिना पृष्ठ लोड के अपडेट करता है।

## साइटमैप

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml` action से एक बार में 1,000 URL माँगता है (`part.offset`, `part.limit`) और XML स्ट्रीम करता है, इसलिए बड़ी साइट कभी मेमोरी में नहीं बैठती। 50,000 URL तक यह एक `urlset` है। उससे ऊपर यह `/sitemaps/1.xml`, `/sitemaps/2.xml` आदि का साइटमैप इंडेक्स है, हर एक में 50,000 URL। action `public` होनी चाहिए और URL को स्थिर क्रम में लौटाना चाहिए। ऐप पथ को `App::site_url` चाहिए, क्योंकि साइटमैप में निरपेक्ष URL होते हैं; जो प्रविष्टियाँ `https` URL या ऐप पथ नहीं हैं वे छोड़ दी जाती हैं। केवल सार्वजनिक पृष्ठ सूचीबद्ध करें: सर्च इंजन दूसरे नहीं खोल सकते। प्रति ऐप एक feature में साइटमैप होता है।

## robots.txt और llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

दोनों सबको सादे टेक्स्ट के रूप में परोसे जाते हैं।

## public फ़ोल्डर

```rust
App::new(t!("app.name"))
    .public(zeyn::public_files!())
```

crate के `public/` फ़ोल्डर की फ़ाइलें `zeyn::public_files!()` से बाइनरी में जुड़ जाती हैं और साइट की रूट से दी जाती हैं: `public/favicon.svg` `/favicon.svg` पर मिलती है, और पेज `/favicon.svg`, `/favicon.png` और `/favicon.ico` में से पहली मिली फ़ाइल को अपने आइकन के रूप में जोड़ता है। चित्र, आइकन, फ़ॉन्ट, टेक्स्ट, XML, JSON, वेब मैनिफ़ेस्ट और PDF फ़ाइलें ली जाती हैं; HTML और स्क्रिप्ट बिल्ड रोक देते हैं, क्योंकि वे ऐप के origin के अंतर्गत चलेंगे। फ़ाइलों के साथ entity tag होता है और वे एक घंटे के लिए कैश होती हैं। जब तक `[package.metadata.zeyn]` में `max_public_file_kb` और `max_public_total_kb` कुछ और न कहें, एक फ़ाइल 256 KB और फ़ोल्डर 2 MB तक हो सकता है। `App::check` उस फ़ाइल की रिपोर्ट करता है जिसका पथ पहले से कोई स्क्रीन, `/robots.txt`, `/llms.txt`, `/sitemap.xml` या `/_zeyn/` इस्तेमाल करता है। `zeyn new` एक `public/favicon.svg` जोड़ता है।
