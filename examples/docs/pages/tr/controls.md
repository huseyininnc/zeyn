# Kontroller ve seçiciler

Form alanları kontrollerini türlerinden seçer. `.control(M::f, Control::...)` onu değiştirir ve `.lookup(M::f, action)` bir alanı sunucu üzerinde aramaya çevirir. Her kontrolün altında yerel bir form öğesi vardır, bu yüzden değerler runtime ile ya da runtime olmadan aynı şekilde gönderilir ve her kontrol klavye ve ekran okuyucu kullanıcıları için kendi ARIA desenini izler.

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## Varsayılan kontroller

| Alan türü | Kontrol |
|---|---|
| `String` | Metin girişi; `max_len` 255'i aştığında karakter sayaçlı, büyüyen bir metin alanı |
| `i32`, `i64` | `min` ve `max` kurallarıyla sayı girişi |
| `Money` | Kullanıcının sayı biçiminde tutar, para birimiyle birlikte |
| `Decimal` | Kullanıcının biçiminde, alanın `scale` değeriyle kesin sayı ([Decimal girişi](#decimal-and-percent) bölümüne bakın) |
| `bool` | Onay kutusu |
| `#[choice]` enum | Select: aranabilir bir liste açan düğme |
| Bir `#[choice]` enum'unun `Vec` değeri | Çoklu select: çipler ve birkaç seçeneğin seçildiği bir liste |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | Tarih ve saat seçiciler |
| Bir modelin `Vec<M>` değeri | Satır düzenleyici ([Satır düzenleyici](lines.md) bölümüne bakın) |

## Diğer kontroller

| Kontrol | Ne için |
|---|---|
| `Control::Radio` | Birkaç seçenekli, hepsi görünen bir seçim |
| `Control::Segmented` | İki ile beş kısa seçenekli bir seçim |
| `Control::Listbox` | Yazarak arama yapılan uzun bir seçim listesi |
| `Control::Switch` | Bir ayar olarak hemen etkili olan `bool` |
| `Control::Percent` | Yüzde işaretiyle gösterilen `Decimal` ya da tam sayı |
| `Control::Password`, `Control::NewPassword` | Göster ve gizle düğmeli, parola olarak yazılan metin alanı |
| `Control::PasswordMeter` | Güç göstergeli yeni parola |
| `Control::Rating` | 1 ile N arasında bir tam sayı, yıldızlar olarak gösterilir; `Decimal` alanda yarım yıldız ([Rating](#rating) bölümüne bakın) |
| `Control::Multiline` | Büyüyen bir metin alanında her uzunlukta metin |
| `Control::Color { palette }` | Seçicili, onaltılık renk tutan bir `String` ([Color and length](#color-and-length) bölümüne bakın) |
| `Control::Length { units }` | Piksel olarak saklanan, kullanıcının seçtiği birimde gösterilen bir `Decimal` uzunluk ([Color and length](#color-and-length) bölümüne bakın) |
| `Control::Code(lang)` | Renklendirme, öneri ve hata işaretleriyle SQL ya da bir ifade ([Kod düzenleyici](#code-editor) bölümüne bakın) |

## Rating

`Control::Rating`, tam sayı bir alanı 1'den N'ye kadar yıldızlar olarak gösterir; N, alanın `max` kuralıdır (kural yoksa 5, en fazla 10). Etiketi alanın etiketi olan bir `fieldset` içindeki yerel radio düğmelerinden oluşan bir gruptur, bu yüzden betik olmadan W3C APG radio group desenini izler: Tab gruba bir kez ulaşır, ok tuşları hareket ettirir ve işaretler, her yıldız kullanıcının dilinde "3 / 5" gibi adlandırılır. Üzerine gelmek, imlecin olduğu yıldıza kadar olanları aydınlatır. Sunucu değeri alanın kurallarıyla her sayı gibi doğrular, bu yüzden alana `min` ve `max` verin; `Option<i64>` üzerinde `required` varsa tüm yıldızları boş bırakmak reddedilir. `#[rule(min = 1, max = 5, scale = 1)]` kuralı olan bir `Decimal` alan yarım yıldız alır; sunucu 4.3 gibi iki yarım arasındaki bir değeri reddeder ([Mağaza bileşenleri](commerce.md) sayfasına bakın).

```rust
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

Form::of(Feedback::default())
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit(t!("feedback.send"), send_feedback)
```

@live rating


## Color and length

```rust
const BRAND: &[&str] = &["#1E88E5", "#43A047", "#00000080"];

Form::of(band)
    .field(Band::fill)
    .control(Band::fill, Control::Color { palette: BRAND })
    .field(Band::height)
    .control(Band::height, Control::Length { units: &[LengthUnit::Mm, LengthUnit::Pt] })
```

`Control::Color`, bir `String` alanı `#RRGGBB` ya da `#RRGGBBAA` olarak tutar. Alan, renk örneği olan bir metin girişidir, bu yüzden betik olmadan da çalışır; düğmesi kırmızı, yeşil, mavi ve opaklık kaydırıcıları, son kullanılan renkler ve uygulamanın paleti olan bir iletişim kutusu açar. Escape ya da Tamam onu kapatır ve odağı geri verir. Sunucu onaltılık renk olmayan bir değeri reddeder.

`Control::Length`, bir `Decimal` alanı CSS pikseli olarak tutar (inç başına 96). Kullanıcı milimetre, santimetre, inç, punto ya da piksel yazar ve birimi sayının yanından seçer; tarayıcı birimi her uzunluk alanı için hatırlar. Yukarı ve Aşağı ok 0,1, Shift ile 1 adım atar; sınır, piksel cinsinden `min` ve `max` kurallarıdır. Alana `scale = 4` verin.

## Lookup

Lookup, kullanıcı yazdıkça kayıtları sunucuda arar. Action bir `LookupQuery` alır ve `Page<LookupItem>` döndürür; alan id'yi saklar.

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)` aynısını bir `Vec<i64>` alanı için yapar ve seçilenleri kaldırılabilir etiketler olarak gösterir.

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` listede birkaç sütun gösterir. Arama yine sunucudaki action'a gider ve permission'ı her tuş vuruşunun aramasında denetlenir; her öğe değerlerini başlıkların sırasına göre `LookupItem::cells` ile verir. `LookupQuery::apply` hücreleri de arar. Ekran okuyucular her seçeneği "Kod: C-2, Ad: Mert Kaya, Şehir: Ankara" olarak duyar. Dar ekranlarda sütunlar her seçeneğin içinde üst üste dizilir.

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## Çoklu select

Bir `#[choice]` enum'unun `Vec` değeri çoklu select ile düzenlenir: seçilen seçenekler kaldırma düğmeli çipler olarak görünür ve bir combobox seçeneklerin açılıp kapatıldığı bir liste açar (`aria-multiselectable` ile APG listbox, bir combobox içinde). Altında yerel bir `<select multiple>` vardır, bu yüzden alan runtime olmadan da çalışır. Runtime değerleri virgülle birleştirilmiş olarak gönderir (`Email,Sms`); sunucu onları tek tek okur, bilinmeyen değerleri reddeder ve tekrarları atar. JSON'da (API'ler, AI araçları, veritabanı) alan, seçenek değerlerinden oluşan bir dizidir.

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| Tuş | Ne yapar |
|---|---|
| Aşağı ok, Yukarı ok, Enter, Boşluk | Kapalı combobox'ta listeyi açar |
| Home, End | Listeyi ilk ya da son seçenekte açar |
| Aşağı ok, Yukarı ok | Açık listede gezdirir |
| Shift ile Aşağı ok ya da Yukarı ok | Gezdirir ve sonraki seçeneği açıp kapatır |
| Boşluk, Enter | Etkin seçeneği açıp kapatır; liste açık kalır |
| Harfler | Onlarla başlayan sonraki seçeneğe atlar |
| Backspace, Delete | Kapalı combobox'ta: son seçilen seçeneği kaldırır |
| Escape, Alt ile Yukarı ok | Listeyi kapatır; odak combobox'ta kalır |
| Tab | Listeyi kapatır ve ilerler |

Ctrl+A (APG desenindeki tümünü seç) kısayol politikasının gerektirdiği gibi tarayıcıya bırakılır; çiplerin kaldırma düğmeleri sekme sırasındadır. Liste, çipler ve lookup sütunları yaklaşık 1,7 KB gzip betik gerektirir ve istek yapmaz; alan 3 node artı seçilen her seçenek için bir node'dur.

## Decimal ve yüzde

`Decimal` alanları kesin sayıları tutar: girişten sunucuya, veritabanına ve dışa aktarıma kadar hiçbir yerde kayan nokta yoktur. Giriş, yazılırken sayıyı kullanıcının dilinde biçimler (Türkçede `1.234,5678`, İngilizcede `1,234.5678`) ve alanın `scale` kadar ondalığına izin verir; sunucu onu aynı dilde okur ve `scale`, `precision`, `min` ve `max` değerlerini yeniden denetler.

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent` değeri yüzde puanı olarak tutar: `12.5` İngilizcede `12.5 %`, Türkçede `%12,5` olarak gösterilir ve `12.5` olarak saklanır. Ekran okuyucular "percent" (yüzde) sözcüğünü girişin açıklaması üzerinden duyar.

## Maskeli giriş

`mask` kuralı olan bir metin alanı, kullanıcı yazarken desenini gösterir ve yalnızca ham karakterleri gönderir. Kural desenin kendisidir: sunucu yazılan sabit karakterleri çıkarır (`(532) 123 45 67` değeri `5321234567` olur), `A` yuvalarını büyük harfe çevirir ve her yuvayı doldurmayan değeri reddeder. API çağrıları ve AI araçları ham değeri gönderir ve aynı denetimi alır.

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| Desen karakteri | Anlamı |
|---|---|
| `9` | Bir rakam |
| `a` | Bir harf |
| `A` | Bir harf, büyük harf olarak yazılır |
| `*` | Bir harf ya da bir rakam |
| ters eğik çizgi | Sonraki karakter sabittir, örneğin `\T` |
| başka her şey | Sabit karakter: gösterilir, hiçbir zaman saklanmaz |

Runtime olmadan alan, ham değerli düz bir metin girişidir; kullanıcılar onu sabit karakterlerle ya da onlarsız yazabilir.

## Parola

`Control::Password` (oturum açma), `Control::NewPassword` (kayıt, değiştirme) ve `Control::PasswordMeter` (güç göstergeli yeni parola), sunucudan hiçbir zaman doldurulmayan ve taslaklarda hiçbir zaman tutulmayan parola girişleridir. Göster ve gizle düğmesi APG toggle button desenini izler: adı "Show password" olarak kalır ve `aria-pressed` metnin görünüp görünmediğini söyler. Form gönderildiğinde parolayı yeniden gizli yapar.

Gösterge, tarayıcıda uzunluktan, karakter türlerinden ve bariz desenlerden hesaplanan bir ipucudur; dört çubuk ve kibar bir live region içinde bir metin ("Strength: good") olarak gösterilir. Sunucunun neyi kabul ettiğine alanın kuralları (`min_len`, bir model `validate` fonksiyonu) karar verir, gösterge asla.

@live inputs

### Klavye

| Tuş | Nerede | Ne yapar |
|---|---|---|
| Yukarı ok, Aşağı ok | decimal ve yüzde girişleri | `min` ve `max` içinde bir tam birim adım atar |
| Page Up, Page Down | decimal ve yüzde girişleri | On adım atar |
| Backspace | maskeli giriş | İmlecin önündeki karakteri siler; sabit bir karakterin üzerindeyse sabit karakterin önündeki karakteri siler |
| Tab | parola girişi | Göster ve gizle düğmesine geçer |
| Enter, Boşluk | göster ve gizle düğmesi | Parolayı gösterir ya da gizler; odak düğmede kalır |

### Bütçe ve denetimler

| | |
|---|---|
| Eklenen betik | `inputs.js` içinde maskeler, anahtar ve gösterge için yaklaşık 2 KB gzip (7 KB ham); decimal ve yüzde girişleri sayı kontrolünü kullanır |
| DOM node'ları | maskeli giriş için 3, anahtarlı parola için 4, göstergeyle 11 |
| İstekler | yazarken yok |
| Sunucu denetimleri | `scale`, `precision`, `min`, `max` ve `mask` her kayıtta çalışır; sayfadan, API'den ve AI araçlarından aynı şekilde |

## Kod düzenleyici

`Control::Code(lang)` bir metin alanını SQL (`CodeLang::Sql`), `[Orders.Total] * 1.18m` gibi rapor ifadeleri (`CodeLang::Expression`) ya da düz metin için bir kod düzenleyiciye çevirir. `CodeEditor` aynı düzenleyicinin model formu dışında kendi başına bir node olan hâlidir. `.complete(action)` öneri, `.check(action)` hata işareti ekler.

```rust
Form::of(calc)
    .control(Calc::formula, Control::Code(CodeLang::Expression))
    .complete(Calc::formula, complete_fields)
    .check(Calc::formula, check_formula)

CodeEditor::new("query", CodeLang::Sql).label("Report query").value(sql).complete(complete_sql)
```

Giriş düz bir `<textarea>` olduğu için yazma, geri alma, giriş yöntemleri, mobil klavyeler ve ekran okuyucular her metin alanındaki gibi çalışır ve form düz metin gönderir. Altında metnin renklendirilmiş bir kopyası ve satır numarası sütunu durur; ikisi de ekran okuyuculardan gizlidir. Renklendirme satır bazlıdır: bir tuş vuruşu yalnızca değiştirdiği satırları boyar. Betik olmadan alan düz bir metin alanıdır.

Tamamlama action'ı bir `CodeQuery` alır (`text` ve `cursor`; yazılan sözcük için `word()`) ve `Vec<Completion>` döner; seçilenin `insert` değeri o sözcüğün yerine geçer. Denetim action'ı aynı sorguyu alır ve `line`, `column`, `length` ve `message` taşıyan `Vec<CodeError>` döner; her hatanın altı çizilir ve düzenleyicinin altında listelenir, ekran okuyucular onu orada duyar. Bir model `validate` fonksiyonunda `errors.add_at(field, line, column, length, message)` da yerin altını çizer. Metnin dışına düşen işaretler metnin içinde tutulur.

### Klavye

| Tuş | Ne yapar |
|---|---|
| Ctrl+Boşluk | İmleçteki önerileri gösterir |
| Aşağı ok, Yukarı ok | Öneriler arasında gezer |
| Enter, Tab | Etkin öneriyi ekler |
| Escape | Önerileri kapatır |
| Tab (öneri açık değilken) | Her metin alanındaki gibi odağı düzenleyiciden çıkarır |

### Bütçe ve denetimler

| | |
|---|---|
| Eklenen betik | `codeedit.js` yaklaşık 8,0 KB gzip ve `codeedit.css` 1,4 KB gzip, yalnızca düzenleyici olan sayfalarda |
| Yazma | 2.000 satırlık bir metinde tuş başına yaklaşık 0,5 ms betik |
| İstekler | tuş başına yok; tamamlama 150 ms, denetim 500 ms duraksamadan sonra, eski istekler iptal edilir |
| Sunucu denetimleri | her istekte CSRF, action'ın izni ve yazma sınırı; önce istek gövdesi okunur (varsayılan gövde sınırıyla en fazla 2 MB), sonra 64 KB üstü metinler action çalışmadan reddedilir; tamamlama ve denetimler kayıtlar gibi `App::write_limit` sınırına sayılır; tarayıcıya en fazla 200 karakterlik 50 öneri ve 500 karakterlik 100 hata ulaşır |

## Tarih ve saat seçiciler

Seçiciler kullanıcının biçiminde yazılan girdiyi alır ve bir takvim ya da saat listesi açar. `DateOptions` sınırları belirler: `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`. `.range(from, to)` iki tarih alanını *Bu ay* gibi hazır ayarlarla tek bir aralık seçicide birleştirir. Canlı örnek [Saat ve diller](time-and-languages.md#pickers) sayfasındadır.
