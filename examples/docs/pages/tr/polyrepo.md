# Ayrı depolarda uygulamalar ve paylaşılan kütüphaneler

Zeyn tek bir depoda çalışır, ama birçok ekip her uygulamayı kendi deposunda tutar ve paylaşılan kodu (entity'ler, servisler, bütün feature'lar) birkaç uygulamanın kullandığı kütüphanelere koyar. Bu rehber bunun nasıl kurulacağını ve her şeyi çalışır tutan tek kuralı gösterir.

## Ne nereye gider

| Depo | Şununla oluşturulur | İçeriği |
|---|---|---|
| Uygulama | `zeyn new billing` | `main.rs`, `app.rs`, uygulama feature'ları, yapılandırma, uygulama katalogları |
| Paylaşılan kütüphane | `zeyn new acme-domain --lib` | entity'ler, servisler, feature'lar, katalogları, `CATALOGS` |

Bir kütüphane feature'ı bir uygulama feature'ı ile aynıdır: model'ler, action'lar, screen'ler, izinler ve çeviriler tek bir klasörde birlikte yaşar. Hangi kütüphane feature'larının bağlanacağına (mount) uygulama karar verir.

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

Kütüphane katalogları önce yüklenir, bu yüzden bir uygulama aynı anahtarı tanımlayarak bir kütüphane metnini geçersiz kılabilir. Kütüphane anahtarlarını feature adıyla ön eklendirin (`customers.list`), böylece iki kütüphane hiçbir zaman çakışmaz.

`examples/shared-domain` böyle bir kütüphanedir. `examples/billing` ve `examples/crm` ikisi de onun customers feature'ını bağlar; crm kendi kataloğunda "Customers" adını "Accounts" olarak değiştirir.

## Kütüphanenin sahip olması gerekenler

Rust'ın orphan kuralı, bir trait'in yalnızca o trait'i ya da türü tanımlayan crate'te uygulanabileceğini söyler. Bir uygulama, bir kütüphanedeki tür için `zeyn::Model` ya da `zeyn::Entity` uygulayamaz, bu yüzden:

- `#[entity]`, `#[model]` ve `#[choice]` özniteliklerini kütüphanedeki türün üzerine, kurallarının yanına koyun.
- Doğrulama kurallarını (`#[rule(...)]`) paylaşılan türde tutun. Böylece her uygulama aynı şekilde doğrular.
- Ek davranışa ihtiyaç duyan bir uygulama türü sarar (`struct Invoice { customer: Customer, ... }`) ya da kendi trait'ini ekler. Kütüphane türünü düzenlemez.

## Uygulamalar kütüphaneyi nasıl alır

| Seçenek | Artısı | Dikkat edilecek |
|---|---|---|
| Etiketli (tag) Git bağımlılığı | Çalıştırılacak bir şey yok, her Git sunucusuyla çalışır | Cargo tüm depoyu getirir; her uygulama URL'yi listeler |
| Özel Cargo registry (Kellnr, GitLab package registry, diğerleri) | Normal `version = "0.3"` bağımlılıkları, semver çözümlemesi | Çalıştırılacak, yedeklenecek ve güvenceye alınacak bir servis |
| Her şey için tek bir workspace deposu | Tek derleme, tek `Cargo.lock`, tek commit'te refactor'lar | Her ekip her uygulamayı görür; erişim denetimi klasör başına değil depo başınadır |

Git etiketleriyle başlayın. Bir elin parmaklarından fazla kütüphaneniz olduğunda ya da sürüm çözümlemesini elle yapmak yorucu hale geldiğinde bir registry'ye geçin. Uygulama kodu değişmez; yalnızca `Cargo.toml` satırları değişir.

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new` varsayılan olarak `zeyn = "<version>"` yazar. Zeyn yayımlanana kadar, her yeni uygulamada ve kütüphanede bu satırı yukarıdaki Git biçimine değiştirin.

## Kural: her yerde tek Zeyn sürümü

Her kütüphane ve her uygulama tam olarak aynı Zeyn'e bağımlı olmalıdır. Git bağımlılıklarında bu, aynı URL ve aynı etiket demektir.

Cargo iki farklı Zeyn kaynağını iki farklı crate olarak görür. `v0.0.2` etiketine karşı derlenmiş bir kütüphane, `v0.0.3` etiketine karşı derlenmiş bir uygulamanın kabul edemeyeceği bir `zeyn::Feature` döndürür ve derleyici aynı ada sahip iki şey arasında bir tür uyumsuzluğu bildirir. Bir registry bunu "aynı semver uyumlu sürüm" düzeyine gevşetir, ama kural kalır: derleme başına tek Zeyn.

Zeyn'i yükseltmek için:

1. Her kütüphanede etiketi güncelleyin, testlerini çalıştırın, yeni bir kütüphane etiketi yayımlayın.
2. Her uygulamada Zeyn ve kütüphane etiketlerini birlikte, tek commit'te güncelleyin.

`cargo tree -d -i zeyn` yinelenenleri listeler. Birden fazla Zeyn yazdırırsa, bir bağımlılık hâlâ eski bir etikete işaret ediyordur.

## Adım adım

1. Kütüphaneyi oluşturun ve kendi deposuna gönderin.

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. Etiketiyle birlikte uygulamanın `Cargo.toml` dosyasına ekleyin ve yukarıda gösterildiği gibi kataloglarını, servislerini ve feature'larını bağlayın.

3. Uygulamanın CI'sının kütüphaneyi okumasına izin verin. Üretilen `.gitlab-ci.yml`, `https://<gitlab host>/` ve `ssh://git@<gitlab host>/` URL'lerini iş token'ını kullanacak şekilde yeniden yazar. GitLab'da kütüphane projesini açın, sonra CI/CD ayarlarında Job token permissions bölümüne gidin ve uygulama projesini izin listesine ekleyin.

4. Bir uygulama testinde `app.check()` çalıştırın (üretilen `tests/app.rs` bunu yapar). Bağlanan bir feature'ın yapısal sorunları olduğunda, örneğin bir dilde eksik bir çeviri anahtarı varsa, başarısız olur.

## Bir kütüphane ve bir uygulama üzerinde aynı anda çalışmak

`Cargo.toml` dosyasını düzenlemeden uygulamayı yerel checkout'unuza yönlendirin. Bunu uygulamanın `.cargo/config.toml` dosyasına koyun ve o dosyayı Git dışında tutun:

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

Değişiklik bittiğinde bunu kaldırın, kütüphaneyi etiketleyin ve uygulamadaki etiketi yükseltin.

## Bir kütüphaneyi yayımlamak

1. `Cargo.toml` içindeki `version` değerini yükseltin.
2. Kuruluysa `cargo semver-checks` çalıştırın. Çağıranları bozan genel API değişikliklerini bildirir. Kütüphane 1.0'ın altındayken kırıcı bir değişiklik yeni bir minor sürüm gerektirir.
3. Commit'leyin, `v<version>` etiketini oluşturun, etiketi gönderin. Etiket ile `Cargo.toml` uyuşmadığında CI işi `tag-matches-version` başarısız olur.
