# Gömülü asistan

Durum: `zeyn`'in `assistant` Cargo feature'ının arkasında uygulandı.

Bir Zeyn uygulaması kendi yapay zeka asistanını taşıyabilir: her sayfada, uygulamanın verisiyle cevap veren ve uygulamanın screen'lerini ve API'lerini oturum açmış kullanıcı olarak çalıştıran bir sohbet. Zeyn ile geliştirilen uygulamalar bunu kendi müşterileri için açabilir, müşterilerin istediği modeli bağlayabilir ve sohbet geçmişini ihtiyaç duydukları yerde tutabilir.

```toml
zeyn = { version = "0.0.2", features = ["assistant"] }
```

Feature olmadan hiçbiri derlenmez: HTTP istemcisi yok, ek route yok, ek bellek yok.

## 1. Kurulum

```rust
use zeyn::assistant::{Assistant, OpenAiCompatible};

let model = OpenAiCompatible::new(config.require("assistant.base_url")?, config.require("assistant.model")?)
    .api_key(config.secret("assistant.api_key")?);

App::new("Lifts")
    .assistant(
        Assistant::new(model)
            .permission("lifts.assistant.use")
            .instructions("You help service engineers plan elevator maintenance.")
            .suggest(work_orders, "Which lifts are overdue for service?")
            .suggest_anywhere("What can you do?"),
    )
```

Hepsi bu kadar. Her sayfa, sohbeti yan panel olarak açan yüzen bir başlatıcı alır. Bir screen sohbeti kendisi de gösterebilir:

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` diğer içeriğin yanına sığar. `Assistant::launcher(false)` yüzen başlatıcıyı kaldırır; böylece sohbet yalnızca bir screen'in yerleştirdiği yerde görünür.

| Seçenek | Varsayılan |
|---|---|
| `permission("...")` | oturum açmış her kullanıcı |
| `instructions("...")` | yok; uygulamanın geliştiricileri yazar, kullanıcılar asla yazmaz |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | başlangıç çipi yok |
| `speech_to_text(p)`, `text_to_speech(p)` | tarayıcının Web Speech API'si |
| `no_voice()` | mikrofon ve sesli okuma açık |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | asistan onaydan sonra kayıt da yapabilir |
| `rate_limit(RateLimit)` | kullanıcı başına dakikada 20 istek |
| `token_limit(Some(RateLimit))` | kullanıcı başına saatte 200.000 token |
| `limits(Limits { .. })` | bkz. 6. bölüm |

## 2. Modeller

`LlmProvider` uygulayan her tür çalışır: akışlı (streamed) bir tur başlatan ve `ChatEvent`'lerden (metin parçaları, araç çağrıları, kullanım) oluşan sınırlı bir kanal döndüren tek bir metot. Yerleşik olanlar:

| Sağlayıcı | Kapsadığı |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI (`https://api.openai.com/v1`), Azure OpenAI v1 API (`https://NAME.openai.azure.com/openai/v1`, deployment model olarak, `api-key` header'ı için `.azure()`), Ollama (`http://localhost:11434/v1`), vLLM, LiteLLM ve uyumlu diğer sunucular. `max_completion_tokens` bilmeyen sunucular için `.legacy_max_tokens()` |
| `Anthropic::new(model)` | Anthropic Messages API (bir ağ geçidi için `.base_url(...)`). Akıl yürütme blokları bir tur içinde değiştirilmeden geri gönderilir |
| `ScriptedProvider` | Testler, demolar ve çevrimdışı geliştirme: ağ çağrısı yerine kurallar, kelime kelime akış, gerçek araç çağrıları |

Anahtarlar `Secret` değerlerinden (`Config::secret`) gelir, sunucuda kalır ve hata mesajlarında asla görünmez. Sağlayıcı hataları sunucuda loglanır; kullanıcı genel bir mesaj görür.

Showcase'i gerçek bir modele yöneltmek yalnızca yapılandırma gerektirir:

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

ya da `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5` ile `NORTHWIND_ASSISTANT__PROVIDER=anthropic`, ya da anahtarsız bir Ollama URL'si. `NORTHWIND_ASSISTANT__PROVIDER=off` asistanı kapatır.

## 3. Konuşma

Varsayılan olarak mikrofon tarayıcının konuşma tanımasını, "sesli oku" ise tarayıcının konuşma sentezini kullanır: sunucu maliyeti yok, uygulama için tarayıcıdan hiçbir ses çıkmaz. Sunucu tarafı konuşma için:

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

İkisi de OpenAI uyumlu `/audio/transcriptions` ve `/audio/speech` uç noktalarıyla konuşur; yerel Whisper ve konuşma sunucuları da bunları sunar. Kayıtlar sınırlıdır (tarayıcıda 4 MB, 60 saniye); konuşma sesi geldikçe tarayıcıya akar.

## 4. Geçmiş

`ChatStore` konuşmaları kiracı ve kullanıcıya göre (`ChatKey`) anahtarlayarak tutar; böylece kiracılar hiçbir zaman aynı konuşmayı paylaşmaz:

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` varsayılandır (her biri 60 iletilik 1.000 konuşma, en az yakın zamanda kullanılan önce çıkar, yeniden başlatmada kaybolur). Bir veritabanı için, kiracı, kullanıcı, sıra ve JSON olarak ileti içeren bir tablo üzerinde üç metodu uygulayın. İletiler yalnızca eklenir; saklama süresine depo karar verir. Bekleyen onaylar da geçmişte durur, bu yüzden sunucu kullanıcı başına başka hiçbir durum tutmaz.

## 5. Araçlar ve action'lar

Model, MCP ve WebMCP ile aynı araçları görür (bkz. `docs/runtime-protocol.md` 9. bölüm); bunlar her seferinde oturum açmış kullanıcı için oluşturulur: kullanıcının kullanabileceği grid aramaları, form kayıtları ve API action'ları, `ai = false` işaretli her şey hariç; açıklamalar `///` doc yorumlarından ve `ai = "..."` yönlendirmesinden gelir. İki yerleşik araç eklenir:

- `navigate`: kullanıcının açabileceği, route'lu bir screen'i sayfa yüklemeden açar (`zeyn:navigate`) ve grid'ini filtreleyebilir (`zeyn:filter`). Grid sonra `POST /_zeyn/grid` üzerinden yüklenir; sunucu orada her zamanki denetimlerini uygular.
- `suggest`: çip olarak gösterilen devam istemleri.

Her çağrı, bağlamın bir ajan olarak işaretlendiği normal action yolundan geçer: screen'in erişimi, Feature'ın guard'ları, action'ın izni, interceptor'lar, hız sınırları, zaman sınırı, denetim kaydı (audit; her asistan çağrısı `via_agent: true` ile kaydedilir) ve maskeleme.

**Onay.** Okumalar hemen çalışır. Yazan her şey (form kayıtları, girdisi `DataQuery` olmayan API action'ları) ve `#[action(confirm)]` işaretli her action turu durdurur ve nelerin çalışacağını, argümanlarıyla birlikte gösteren bir onay kartı sunar. Yalnızca kullanıcının tıklaması (`POST /_zeyn/assistant/confirm`, CSRF ile, o kullanıcının kendi geçmişinde bekleyen bir çağrı için) onu bir kez çalıştırır. "Onaylıyorum" gibi bir ileti yalnızca metindir: yeni bir ileti bekleyen çağrıyı çalıştırılmadı olarak kapatır.

## 6. Güvenlik modeli

| Risk | Framework'ün yaptığı |
|---|---|
| Sızan API anahtarları | Anahtarlar sunucuda `Secret`'tır; tarayıcı yalnızca uygulamayla konuşur |
| Kullanıcının ötesinde davranan bir model | Araçlar kullanıcının izinlerinden oluşturulur; her çağrı sunucuda yeniden denetlenir |
| Kullanıcının istemediği yazmalar | Yazmalar ve `confirm` action'ları kullanıcının tıklamasını gerektirir; model onaylayamaz |
| Maskeli verinin modele ulaşması | Maskeli alanlar, açığa çıkarma izni olan kullanıcılar için bile ajan çağrılarında her zaman maskelidir |
| Veri üzerinden istem enjeksiyonu (prompt injection) | Araç sonuçları, `<` ve `>` kaçırılmış olarak `<tool_result>` sınırlayıcıları içinde düz metindir; sistem istemi bunların veri olduğunu söyler. Sistem istemi her turda sunucu tarafından sabit kurallardan, uygulamanın talimatlarından ve screen adından yeniden oluşturulur; kullanıcı, sayfa ya da araç verisinden asla |
| Bağlantılar üzerinden sızdırma | Cevaplar `textContent` ile oluşturulan küçük bir Markdown alt kümesi olarak işlenir: HTML yok, bağlantı yok, resim yok |
| Kontrolsüz maliyet | Kullanıcı başına istek hız sınırı, kullanıcı başına token sınırı, sınırlı çıktı token'ları, tur başına en fazla 6 tur ve 4 araç çağrısı |
| Büyük girdiler | 2.000 karaktere kadar iletiler, tur başına 30 geçmiş iletisi, 6.000 karaktere kesilen araç sonuçları, 4 MB'a kadar kayıtlar |
| Siteler arası istekler | Her `POST` CSRF header'ı gerektirir ve uygulamanın yazma sınırına sayılır |
| Hesap verebilirliği olmayan işlemler | Her asistan araç çağrısı `via_agent` ile denetlenir |

Yapmadığı şey: bir modelin cevabının doğru olup olmadığına karar vermek. Kullanıcılara bunu söyleyin (panel söyler) ve önemli action'ları `confirm` arkasında tutun.

## 7. Müşteriler için gömme

Asistanı kendi müşterilerine satan bir uygulama genellikle şunları ister:

- `permission("app.assistant.use")` ve bir guard içinde lisans modülü denetimi; böylece yalnızca ödeme yapan kiracılar görür;
- kiracı başına model: müşterinin sağlayıcısını `ctx` kiracısına göre yapılandırmadan seçen küçük bir yönlendirici olarak `LlmProvider` uygulayın ya da herkes için tek bir model çalıştırın;
- müşterinin saklama kurallarıyla, uygulamanın veritabanında bir `ChatStore`;
- alanı anlatan `instructions(...)` (bir asansör şirketi: üniteler, siteler, servis aralıkları) ve ihtiyaç duyan action'larda `ai = "..."` yönlendirmesi;
- müşterinin ödediği plana göre ayarlanmış `token_limit`.

## 8. Uç noktalar

| Uç nokta | Yaptığı |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | `application/x-ndjson` olaylarını akıtır |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | Bekleyen çağrıyı çalıştırır veya reddeder, sonra sohbet gibi akıtır |
| `POST /_zeyn/assistant/clear` | Konuşmayı siler; `204` |
| `POST /_zeyn/assistant/stt` (ses gövdesi) | `{ text }`; sunucu tarafı konuşmadan metne yoksa `404` |
| `POST /_zeyn/assistant/tts` `{ text }` | Akışlı ses; sunucu tarafı metinden konuşmaya yoksa `404` |

Olaylar, satır başına bir JSON nesnesi: `{t:"text",v}`, `{t:"tool",name,id}`, `{t:"navigate",url,screen,filters}`, `{t:"suggest",v}`, `{t:"confirm",id,tool,title,question,details}`, `{t:"resolved",id,approved}`, `{t:"error",message}`, `{t:"done"}`.

`assistant.js` ve `assistant.css` yalnızca asistanı kullanabilen kullanıcılar için yüklenir. Sohbet günlüğü bir `role="log"` canlı bölgesidir (cevap akarken `aria-busy`), Enter gönderir, Shift+Enter yeni satır başlatır, Escape yan paneli kapatır ve odağı başlatıcıya döndürür; onay kartı odağı önce İptal'e koyar.

## 9. Maliyet

Asistan, sınırlı geçmiş deposu ve uçuştaki akışlar dışında bellekte kullanıcı başına hiçbir durum tutmaz. Cevaplar 32 öğelik kanallar üzerinden uçtan uca akar. Showcase'de ölçüldü (debug derlemesi, Windows, beş sayfa yüklemesinden sonra boşta): asistan kapalıyken 14,4 MB çalışma kümesi (working set) ve 3,13 MB özel bellek, açıkken 14,5 MB ve 3,19 MB (yaklaşık 100 KB fazla).
