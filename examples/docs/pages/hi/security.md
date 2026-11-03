# सुरक्षा

Zeyn में सुरक्षा उस कोड के पास घोषित की जाती है जिसकी वह रक्षा करती है, और हर कॉल पर सर्वर पर जाँची जाती है। डिफ़ॉल्ट सख़्त हैं; किसी को ढीला करना `app.rs` में एक दिखने वाली पंक्ति है।

## अनुमतियाँ

हर action और Screen `permission = "feature.resource.verb"` या `public` घोषित करती है। इसके बिना वह कंपाइल नहीं होती। अनुरोध की आइडेंटिटी अपनी अनुमतियाँ रखती है:

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

जिन मेनू आइटम, टूलबार बटन, पंक्ति action और कमांड को उपयोगकर्ता नहीं चला सकता वे HTML से हटा दिए जाते हैं। उन्हें हटाना एक सुविधा है; सर्वर हर कॉल पर अनुमति फिर जाँचता है।

## साइन इन

ऐप पासवर्ड नहीं रखता। `Authenticator` पता लगाता है कि कौन कॉल कर रहा है, आमतौर पर ऐप के सामने के सिंगल साइन ऑन प्रॉक्सी से:

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

`ForwardAuth` तभी इस्तेमाल करें जब हर अनुरोध प्रॉक्सी से गुज़रे; जो भी ऐप तक सीधे पहुँच सकता है वह ये हेडर भेज सकता है। `require_header` प्रॉक्सी द्वारा जोड़ा गया साझा सीक्रेट जाँचता है। `DevAuth::superuser(name)` स्थानीय डेवलपमेंट के लिए हर अनुरोध को पूरा एक्सेस देता है, और चालू होने पर सर्वर चेतावनी छापता है। रिलीज़ बिल्ड इसके साथ शुरू होने से मना कर देती है (`App::check` इसे बताता है); जिस डेमो को रिलीज़ में चलना हो वह `App::allow_dev_auth()` कॉल कर सकता है। `zeyn new` से बने ऐप इसे केवल डिबग बिल्ड में चालू करते हैं।

## गार्ड

गार्ड किसी feature की किसी भी Screen के खुलने से पहले और उसके किसी भी ग्रिड, फ़ॉर्म या कमांड के कॉल होने से पहले चलता है:

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

बिल्ट इन गार्ड: `SignedIn`, `TenantRequired` और `Licensed`।

## इंटरसेप्टर

इंटरसेप्टर एक्सेस जाँच के बाद हर Screen, ग्रिड, फ़ॉर्म, API और कमांड कॉल के आसपास चलता है। `before` कॉल रोक सकता है; `after` देखता है कि वह कैसे खत्म हुई। `AuditLog(sink)` बिल्ट इन है जो दर्ज करता है कि किसने क्या किया।

## CSRF, CSP और हेडर

- हर पृष्ठ एक यादृच्छिक `zeyn_csrf` कुकी (`HttpOnly`, `SameSite=Strict`) सेट करता है और वही टोकन एक meta टैग में रखता है। रनटाइम की हर कॉल उसे `x-zeyn-csrf` हेडर में भेजती है और सर्वर दोनों की तुलना constant time में करता है।
- बिना सत्र के खुली सार्वजनिक Screen अपवाद हैं: वे कोई कुकी सेट नहीं करतीं और कोई टोकन नहीं रखतीं, इसलिए वे कैश हो सकती हैं। रनटाइम अपनी पहली कॉल से पहले `GET /_zeyn/csrf` से टोकन लेता है, और हर `POST` अब भी उसी तरह जाँचा जाता है (देखें [सार्वजनिक पृष्ठ](public-pages.md))।
- Content Security Policy स्क्रिप्ट, स्टाइल और छवियों की अनुमति केवल ऐप के अपने origin से देती है। कोई इनलाइन स्क्रिप्ट नहीं हैं; थीम स्टाइलशीट हैं, कभी इनलाइन स्टाइल नहीं।
- `App::csp` नीति को नामित होस्ट के लिए खोलता है, एक बार में एक directive। हेडर स्टार्टअप पर एक बार बनता है।

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  केवल `https://host`, `https://host:port` और `https://*.host` स्वीकार हैं (वाइल्डकार्ड सबडोमेन से मेल खाता है, होस्ट से खुद नहीं)। `.media_blob()` स्ट्रीमिंग लाइब्रेरी के लिए `media-src` में `blob:` जोड़ता है। `App::check` `*`, `http:`, `data:` या `https:` जैसी अकेली स्कीम, `'unsafe-inline'` या `'unsafe-eval'` जैसे उद्धृत कीवर्ड, पथ, उपयोगकर्ता नाम, और खाली जगह, अल्पविराम या अर्धविराम वाले मान अस्वीकार करता है; हर समस्या बताती है कि इसके बजाय क्या लिखें। `script-src`, `object-src`, `base-uri` और `frame-ancestors` बदले नहीं जा सकते। `Embed` और `Image` वही सूचियाँ पढ़ते हैं, इसलिए जो होस्ट ब्राउज़र अवरुद्ध करता वह कभी रेंडर नहीं होता (देखें [डिस्प्ले](display.md))।
- उत्तर `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin` और `X-Frame-Options: DENY` रखते हैं।
- `App::secure_cookies(true)` कुकी को `Secure` चिह्नित करता है; HTTPS के पीछे इसे चालू करें।

## Escaping

HTML में उपयोगकर्ता डेटा हमेशा `render::escape` से गुज़रता है। कंपोनेंट हर रेंडर किया मान escape करते हैं, इसलिए `<script>` नाम का उत्पाद टेक्स्ट के रूप में दिखता है।

## सीमाएँ

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit` स्थिर एसेट और सार्वजनिक मीडिया स्रोतों (`Feature::public_media`) के पठन को छोड़कर हर अनुरोध पर लागू होती है; `write_limit` डेटा बदलने वाली कॉल के लिए एक और सख़्त सीमा जोड़ती है: फ़ॉर्म सबमिट, ग्रिड संपादन, पंक्ति, बल्क और मेनू action, कमांड, इम्पोर्ट, सहेजे गए ग्रिड व्यू, डॉक लेआउट और थीम, API कॉल, MCP कॉल और असिस्टेंट। अस्वीकार किए अनुरोधों को `Retry-After` के साथ `429` मिलता है।
- `X-Forwarded-For` पर केवल `trusted_proxies` से भरोसा किया जाता है। हेडर की हर पंक्ति क्रम से गिनी जाती है, और लिया गया पता वह आखिरी है जो भरोसेमंद प्रॉक्सी नहीं है।
- रिवील, island और डेटा स्रोत कॉल भी लेखन गिने जाते हैं, और निर्यात तथा छपे दस्तावेज़ भी (`GET /_zeyn/grid/export`, `GET /_zeyn/pdf`)।
- पृष्ठ आकार, पंक्ति संख्या, क्वेरी लंबाई और अपलोड आकार क्लाइंट कुछ भी भेजे, सर्वर पर सीमित हैं।

## सत्र

`App::idle_timeout(d)` बिना इनपुट के कुछ समय बाद उपयोगकर्ताओं को पृष्ठ से साइन आउट कर देता है, पहले चेतावनी डायलॉग के साथ। `reauth` चिह्नित action को हाल का साइन इन चाहिए (`App::reauth_within`)।

## डेटा सुरक्षा

फ़ील्ड मास्किंग, संवेदनशील रिकॉर्ड खोलने से पहले कारण, फ़ील्ड स्तर ऑडिट और पठन ऑडिट [ऑडिट, मास्किंग और जॉब](enterprise.md) में हैं।

## API और एजेंट

API कॉल को `Authorization` हेडर चाहिए; अकेली ब्राउज़र सत्र कुकी API या MCP एंडपॉइंट को कभी कॉल नहीं करती, इसलिए उन्हें CSRF टोकन नहीं चाहिए। एजेंट को ठीक उस उपयोगकर्ता की अनुमतियाँ मिलती हैं जिसकी ओर से वह काम करता है। देखें [API और OpenAPI](api.md) और [MCP और AI एजेंट](mcp.md)।
