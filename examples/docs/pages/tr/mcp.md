# MCP ve AI ajanları

Zeyn iki taraftaki ajanlar için tasarlanmıştır: Zeyn uygulamaları yazan ajanlar ve çalışan bir uygulamayı bir kullanıcı adına kullanan ajanlar.

## Uygulama yazan ajanlar

Bir ajanın çalıştığı proje kendi kurallarını ve araçlarını taşır:

- `AGENTS.md` ve `CLAUDE.md` komutları, proje yapısını, dilin kurallarını ve bir framework dizinini listeler.
- `.mcp.json`, stdio üzerinde bir MCP sunucusu olan `zeyn mcp` komutunu şu araçlarla kaydeder:

| Araç | Ne yapar |
|---|---|
| `zeyn_docs` | Konuya göre framework başvurusu: `rules`, `language`, `protocol` |
| `zeyn_describe` | Uygulamanın Feature'ları, ekranları, rotaları, erişimi, API'leri, sorunları ve eksik çevirileri |
| `zeyn_check` | Derler ve sorunları bildirir; `fix: true` önce yeniden adlandırılmış API'leri yeniden yazar |
| `zeyn_test` | `cargo test` çalıştırır ve çıktının sonunu döndürür |
| `zeyn_new_feature` | Bir Feature klasörü oluşturur ve kaydeder |

Ana koruyucu derleyicidir. Zorlayabildiği her kuralın bir `compile_fail` testi ve bir ajanın harekete geçebileceği şekilde yazılmış bir `#[diagnostic::on_unimplemented]` mesajı vardır, örneğin:

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run`, `zeyn_describe` aracının döndürdüğü aynı açıklamayı yazdırır ve çıkar.

## Bir uygulamayı kullanan ajanlar

```rust
App::new(t!("app.name")).agents(Agents::read_only())
```

`agents` ile uygulamanın ekranları araca dönüşür:

- her grid için bir arama aracı,
- her form için, formun alanları ve kuralları şeması olan bir kaydetme aracı (yalnızca `Agents::read_write()` ile).

Ajanlar bunlara iki yoldan ulaşır:

- **Tarayıcıda** WebMCP (`document.modelContext.registerTool`) üzerinden, tarayıcı destekliyorsa. Geçerli ekranın araçları ekran açılırken kaydedilir, kapanırken kaldırılır.
- **Sunucuda** `POST /_zeyn/mcp` adresinde (JSON-RPC). Bir `Authorization` başlığı gerektirir; bu yüzden yalnızca bir tarayıcı oturumu onu asla sürdüremez.

## İnsanlarla aynı kurallar

- Bir ajan, adına hareket ettiği kullanıcının izinlerinin tam olarak aynısını alır. Kullanıcının açamayacağı ekranlar ve action'lar için araçlar listelenmez.
- Çağrılar, normal bir çağrının guard'larını, interceptor'larını, doğrulamasını ve hız sınırlarını çalıştırır.
- Denetim kayıtları ajanların yaptığı çağrıları işaretler; böylece bir gözden geçiren onları ayırt edebilir.
- `Agents::read_only()` ajanların aramasına izin verir ama kaydetmesine izin vermez. `read_write()` seçeneğini yalnızca bir ajanın veriyi değiştirmesi gereken uygulamalar için seçin.
