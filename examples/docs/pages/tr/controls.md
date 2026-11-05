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
| `Control::Rating` | 1 ile N arasında bir tam sayı, yıldızlar olarak gösterilir ([Rating](#rating) bölümüne bakın) |

## Rating

`Control::Rating`, tam sayı bir alanı 1'den N'ye kadar yıldızlar olarak gösterir; N, alanın `max` kuralıdır (kural yoksa 5, en fazla 10). Etiketi alanın etiketi olan bir `fieldset` içindeki yerel radio düğmelerinden oluşan bir gruptur, bu yüzden betik olmadan W3C APG radio group desenini izler: Tab gruba bir kez ulaşır, ok tuşları hareket ettirir ve işaretler, her yıldız kullanıcının dilinde "3 / 5" gibi adlandırılır. Üzerine gelmek, imlecin olduğu yıldıza kadar olanları aydınlatır. Sunucu değeri alanın kurallarıyla her sayı gibi doğrular, bu yüzden alana `min` ve `max` verin; `Option<i64>` üzerinde `required` varsa tüm yıldızları boş bırakmak reddedilir.

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

## Tarih ve saat seçiciler

Seçiciler kullanıcının biçiminde yazılan girdiyi alır ve bir takvim ya da saat listesi açar. `DateOptions` sınırları belirler: `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`. `.range(from, to)` iki tarih alanını *Bu ay* gibi hazır ayarlarla tek bir aralık seçicide birleştirir. Canlı örnek [Saat ve diller](time-and-languages.md#pickers) sayfasındadır.
