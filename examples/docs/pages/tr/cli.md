# CLI ve projeler

`zeyn` komutu proje oluşturur, Feature ekler, bir uygulamayı denetler ve ajanlar için bir MCP sunucusu çalıştırır.

## Komutlar

| Komut | Ne yapar |
|---|---|
| `zeyn new <name>` | Standart Feature yapısıyla bir uygulama oluşturur |
| `zeyn new <name> --lib` | Birkaç uygulamanın paylaştığı model, Feature ve metin kütüphanesi oluşturur |
| `zeyn g feature <name>` | `models`, `services`, `actions` ve `screens` klasörleri ve çalışan bir liste screen'iyle `src/features/<name>` ekler |
| `zeyn g screen <feature> <name>` | Bir feature'a `screens/<name>.rs` ekler, kaydeder ve başlığını her dil dosyasına ekler |
| `zeyn routes` | Feature'ları, screen'leri, rotaları, API'leri ve erişimi listeler |
| `zeyn check` | Derler ve yapısal sorunları ile eksik çevirileri bildirir |
| `zeyn check --fix` | `src`, `tests` ve `examples` içinde adı değişen Zeyn API'lerini yeniden yazar, sonra denetler |
| `zeyn check --json` | Raporu kararlı kodlar, nedenler ve çözümlerle JSON olarak yazar |
| `zeyn mcp` | AI ajanları için stdio üzerinde bir MCP sunucusu çalıştırır |

crates.io üzerinden kurun:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Bu depo içinde CLI'yi Cargo üzerinden çalıştırın:

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## Yeni bir uygulama

`zeyn new`, derlenen, testlerini geçen ve ajanlara verilen kuralları izleyen bir proje yazar:

- `AGENTS.md` ve `CLAUDE.md`: proje kuralları ve ajanlar için bir framework dizini.
- `.mcp.json`: `zeyn` MCP sunucusunu kaydeder; böylece bir ajan `zeyn_check` ve `zeyn_docs` çağırabilir.
- `config/app.json`: varsayılanlar. `APP_*` ortam değişkenleri bunları geçersiz kılar; örneğin `server.addr` için `APP_SERVER__ADDR`.
- `locales/en.json` ve `locales/tr.json`: kullanıcının gördüğü her metin.
- `src/features/`: her Feature için bir klasör, örnek bir `notes` Feature'ı ile.
- `rustfmt.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` ve `Cargo.toml` içindeki `[lints]`: CI'ın zorunlu kıldığı standartlar.
  `rustfmt.toml` `chain_width = 40` ayarlar; böylece kod yukarıdan aşağı okunur: bundan uzun bir çağrı zinciri, satır başına ortalama bir ya da iki nokta olacak şekilde, her çağrı ayrı satıra konur. Ajanlar ve insanlar bir listeyi uzun tek bir satırdan daha hızlı inceler ve diff'ler değişen tek çağrıyı gösterir. Bu ayarı koruyun.

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  şöyle değil: `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`.
- `build.rs` ve `Cargo.toml` içinde `[package.metadata.zeyn]` altındaki `max_file_lines = 300`: `src/` içindeki bir dosya daha fazla boş olmayan satıra sahipse `cargo build` durur; clippy `too_many_lines` (60, `clippy.toml` içinde) aynısını fonksiyonlar için yapar. Bkz. [Project structure](structure.md).
- `Dockerfile` ve `.dockerignore`: küçük bir konteyner imajı.
- Yaygın CI sunucuları için, hepsi aynı denetimleri çalıştıran (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`) ve `Cargo.toml` ile eşleşmeyen bir `vX.Y.Z` etiketini reddeden pipeline'lar:
  - GitHub Actions için `.github/workflows/ci.yml`; yanında `.github/dependabot.yml` (haftalık Cargo ve action güncellemeleri) ve bir pull request kontrol listesi;
  - GitLab için `.gitlab-ci.yml`;
  - Jenkins için `Jenkinsfile` (Docker ajanı, `cargo llvm-cov` ile kapsama).
- `sonar-project.properties`: SonarQube analizi (Clippy sorunları ve `target/lcov.info` içinden LCOV kapsaması). GitHub workflow'u, `SONAR_HOST_URL` depo değişkeni ve `SONAR_TOKEN` sırrı tanımlıysa bunu çalıştırır; Jenkins, iş `SONAR_ENABLED=true` ayarlıyorsa çalıştırır.
- `SECURITY.md` ve `CHANGELOG.md` (Keep a Changelog).
- Git kuruluysa (Windows, macOS ya da Linux) ve klasör zaten bir deponun içinde değilse `main` dalında yerel bir git deposu. Hiçbir şey commit edilmez, bu yüzden ilk commit kendi adınızı ve e-postanızı taşır. `--no-git` bunu atlar.

Her klasör için [Project structure](structure.md) sayfasına bakın.

## Üreticiler

`zeyn g feature` her model, servis, action ve screen için bir dosya içeren bir feature oluşturur:

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

`zeyn g f` ve `zeyn g s` kısa biçimleri aynı işi yapar. Yeni feature hemen çalışır: bir model, bellekte tutulan bir veri servisi, bir arama action'ı ve menü kaydıyla bir liste screen'i. İhtiyaç duymadığı bir klasörü feature siler (klasörü ve feature'ın `mod.rs` dosyasındaki `mod` satırını). `components/` hiçbir zaman üretilmez: bir screen parçası paylaşıldığında ya da bir screen dosyası büyüdüğünde onu elle ekleyin, her dosyada `Node` döndüren bir fonksiyon (`components/<name>.rs`). Bir screen `<feature>.<name>.view` iznini ve `locales/` içindeki her dosyada başlığını alır; İngilizce dışındaki diller İngilizce metni ve çevrilmesi gerektiğini söyleyen bir not alır. Bu düzen zorunlu değildir: bir feature tek bir `screens.rs` kullanıyorsa `zeyn g screen` önce onu içeriğiyle birlikte `screens/mod.rs` konumuna taşır, `screens/` silinmişse yeniden oluşturur. Üreticiler hiçbir dosyanın üzerine yazmaz ve snake case olmayan adları reddeder.

## Bir uygulamayı denetleme

`zeyn check` önce derleyiciyi, sonra uygulamanın kendi denetimlerini çalıştırır (`App::check`): yinelenen rotalar, iki kez kaydedilmiş screen'ler, bir screen'in ihtiyaç duyduğu ama kimsenin kaydetmediği servisler, kullanılamaz kısayollar, WCAG AA kontrastının altındaki tema belirteçleri ve bir dilde eksik çeviri anahtarları. Bir şey yanlışsa hata ile çıkar; böylece CI bunu çalıştırabilir.

Denetimi hiçbir zaman başarısız yapmayan uyarılar da yazar: hiçbir feature'ın kaydetmediği bir `#[screen]` veya `#[action]`, ve yaklaşık 150 satırı geçmiş, içinde birden çok öğe olan tek bir `screens.rs` veya `actions.rs`.

Bir Zeyn sürümü bir API'nin adını değiştirdiğinde `zeyn check --fix` eski adları yeniden yazar, örneğin `Grid` yerine `DataGrid` ve `GridQuery` yerine `DataQuery`, sonra yeniden denetler.

```sh
zeyn check --fix
```

Her sorunun kararlı bir kodu vardır: uygulama, feature'lar, rotalar ve servisler için `Z01xx`, güvenlik için `Z02xx`, herkese açık sayfalar için `Z03xx`, menüler, komutlar ve kısayollar için `Z04xx`, veri için `Z05xx`, sayfa denetimleri için `Z06xx` ve uyarılar için `W01xx`. Metin çıktısı her sorunu nedeni (`why`), çözümü (`help`), bu belgelere bir bağlantı ve koduyla gösterir. `zeyn check --json` aynı raporu ajanlar ve CI için tek bir JSON belgesi olarak yazar ve `ok` false olduğunda hata ile çıkar.

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

## Docker

Oluşturulan `Dockerfile` iki aşamada derler ve root yetkisi olmayan bir kullanıcıyla distroless bir imajda çalışır. İkili dosya kendi sağlığını `<app> health` ile denetler; `HEALTHCHECK` bunu kullanır.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

Oluşturulan bir uygulamanın imajı yaklaşık 43 MB'tır ve boşta çalışan bir uygulama birkaç MB RAM kullanır.

## MCP sunucusu

`zeyn mcp`, stdio üzerinden MCP konuşur. Araçları `zeyn_docs` (konuya göre framework başvurusu), `zeyn_describe` (geçerli dizindeki uygulamanın Feature'ları, screen'leri, rotaları ve sorunları), `zeyn_check` (`fix: true` ile `check --fix` çalıştırır), `zeyn_test`, `zeyn_new_feature` ve `zeyn_new_screen`'dir. [MCP and AI agents](mcp.md) sayfasına bakın.
