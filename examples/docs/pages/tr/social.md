# Sosyal

Sosyal sayfalar için bileşenler. Her biri sunucuda çizilir ve stil dosyasını ve betiğini yalnızca onu kullanan sayfalarda yükler.

## StoryRing

`StoryRing`, bir kişinin hikayelerinin izleyen için yeni olup olmadığını gösteren, halka içindeki bir avatardır. `StoryState::Unseen` marka renklerinde kalın bir halka, `StoryState::Seen` ince ve nötr bir halka çizer. Durum sunucudan gelir, yani uygulamanın izleyici hakkında sakladığı bilgiden; tarayıcı onu hiç değiştirmez. `StoryRow` halkaları, bir ray gibi yana kayan bir satırda toplar.

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

`.link(screen, params)` verilirse halka, hikayeleri gösteren ekrana giden bir bağlantıdır. Erişilebilir adı kişinin adı ve durumudur, örneğin "Ada Lovelace, görülmemiş hikayeler"; halkanın altındaki görünen ad da aynı biçimde başlar, böylece sesle kontrol kullananlar onu söyleyebilir. İki durum yalnızca renkte değil halkanın kalınlığında da ayrışır; zorunlu renkler kipinde görülmüş halka kesikli çizilir. Resim `Avatar` kurallarına uyar (uygulamanın bir yolu veya izin verilen bir `https` adresi); resim yoksa baş harfler gösterilir.

Halka yalnızca CSS ile çizilir: betik yok, resim yok. `stories.css` (2,2 KB) yalnızca halka içeren sayfalarda yüklenir. Bir satır en fazla `MAX_STORY_RINGS` (100) halka alır. Dokunma, izleme dörtgeni ve Shift+tekerlek satırı kaydırır, halkalar yerine oturur ve klavye odağı alan halka görünür alana kayar. Bir satırın hiçbir halkasında bağlantı yoksa odağı satırın kendisi alır; böylece klavye yine de onu kaydırabilir.

| Tuş | Ne yapar |
|---|---|
| Tab | Sonraki halkaya geçer ve onu görünür alana kaydırır |
| Enter | Odaktaki halkanın hikayelerini açar |
| Left, Right | Odak satırın kendisindeyken: satırı kaydırır |
