# Kaynak, lisans ve güvenlik

Zeyn açık kaynaktır. Bu sayfa kodun nerede durduğunu, hangi lisansla dağıtıldığını, nasıl test edildiğini ve bir güvenlik sorununun nasıl bildirileceğini anlatır.

## Kaynak kodu

Kod ve issue'lar GitHub'dadır: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). Crate'ler [crates.io](https://crates.io/crates/zeyn) üzerinde yayımlanır, API başvurusu [docs.rs](https://docs.rs/zeyn) adresindedir.

## Lisans

Zeyn, tercihinize göre [MIT lisansı](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) veya [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE) ile çift lisanslıdır (`Cargo.toml` içinde `MIT OR Apache-2.0`). Katkılar da aynı iki lisansla kabul edilir.

## Kalite

Depoda 837 test fonksiyonu vardır (crate'lerde ve örneklerde `#[test]` ve `#[tokio::test]`, 2026-11-09 tarihinde sayıldı). Bunlara ek olarak doc yorumlarındaki örnekler doctest olarak çalışır.

Her pull request ve `dev` ile `main` dallarına her push CI çalıştırır:

- biçim için `cargo fmt --all --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`, yani her lint uyarısı derlemeyi durdurur;
- `cargo test --workspace`, her crate ve örneğin testleri;
- `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email` ve `accounts` özellikleriyle `cargo test -p zeyn`;
- framework'ü sunucu özelliği olmadan da derleyen `cargo test -p zeyn-cli`;
- lisanslar, güvenlik bildirileri ve yasaklı bağımlılıklar için `cargo deny`.

Her çalışmanın sonucu [Actions sayfasında](https://github.com/huseyininnc/zeyn/actions) herkese açıktır.

## Güvenlik

- **Desteklenen sürümler.** Zeyn alpha aşamasındadır. Güvenlik düzeltmelerini yalnızca en son `0.0.x` sürümü alır.
- **Bildirme.** Bir açığı GitHub Security Advisories üzerinden gizli olarak bildirin: deponun Security sekmesini açıp "Report a vulnerability" seçeneğini seçin ([doğrudan bağlantı](https://github.com/huseyininnc/zeyn/security/advisories/new)). Açıkları asla herkese açık bir issue, tartışma veya pull request içinde bildirmeyin.
- **Yanıt.** Bildirimin alındığı 5 iş günü içinde yanıtlanır. Düzeltme yayımlandıktan sonra bir güvenlik bildirisi yayımlanır ve bildiren kişi, adının geçmesini istemediğini söylemedikçe anılır.

Bildirimde nelerin yer alacağı ve kapsamın ne olduğu dahil politikanın tamamı [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md) dosyasındadır.

## Daha fazlası

- [Değişiklik günlüğü](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): her sürümün değişiklikleri.
- [Yol haritası](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): sonraki sürüm ve planlananlar.
- [Katkı rehberi](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): derleme, test ve değişiklik gönderme.
- [Davranış kuralları](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): birlikte nasıl çalıştığımız.
- [Destek](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): soruların ve hata bildirimlerinin yeri.
