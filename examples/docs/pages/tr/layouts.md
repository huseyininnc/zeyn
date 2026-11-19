# Yerleşimler

Yerleşim (layout) bileşenleri diğer bileşenleri yerleştirir. Veri tutmazlar ve istek eklemezler; bunlarla kurulan bir ekran tek bir HTML yanıtıdır.

```rust
Stack::new()
    .add(Row::new().col(Card::new().title("Orders")).col(Card::new().title("Stock")).col(Card::new().title("People")))
    .add(
        Panel::new("Products").collapsible().add(
            Tabs::new("Views")
                .tab("Card", Card::new().title("A card inside a tab"))
                .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
        ),
    )
```

@live layout

## Stack

Düğümler (node) alt alta. `.gap(units)` aralarındaki boşluğu aralık birimleriyle (normal yoğunlukta 4 px), 0 ile 12 arasında belirler.

## Row

Yan yana sütunlar. Her `.col(node)` genişlikten bir pay alır; bir sütundan sonra `.weight(n)` onu daha geniş yapar. Dar ekranlarda sütunlar üst üste dizilir. `.compact()` rozetler gibi küçük öğeleri sıkıştırır.

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card ve Panel

`Card`, isteğe bağlı başlığı olan düz bir yüzeydir: 20 px iç boşluk, tek kenarlık ve gölge yok. `.action(node)` başlık satırının sonuna tek bir eylem koyar (bir bağlantı düğmesi ya da bir menü), `.footer(node)` bir ayırıcının altına bir alt bilgi ekler, örneğin "Tümünü gör" bağlantısı. Bir kartın içeriği olan grid ya da liste kenardan kenara uzanır, böylece iki değil tek kenarlık olur; kart içindeki bir kart kendi kutusunu bırakır; bir `Row` içindeki kartlar en uzun olanın boyuna uzar. `Panel`, isteğe bağlı araç çubuğu olan başlıklı bir bölümdür; `.collapsible()` kullanıcının onu katlamasına izin verir, `.collapsed()` katlı başlatır.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` ile `.tab(title, node)`. Ok tuşları, ARIA tabs deseninin tarif ettiği gibi sekmeler arasında gezinir. Kapalı bir sekmenin içeriği, sekme ilk açılana kadar render edilmez; böylece çok sekmeli bir ekran hafif kalır.

`.remember(KEY)`, kullanıcının açtığı sekmeyi bir choice anahtarı altında tutar (bkz. [İstemci tercihleri](security.md#client-preferences)); böylece sonraki sayfalarda aynı sekmeler onun üzerinde açılır. URL karmasında adı geçen sekme önceliklidir.

## Accordion

`Accordion::new()` ile `.section(title, node)`, bölümleri alt alta koyar; her biri başlığıyla açılıp kapanır: bir yardım sayfasının cevapları, uzun bir kaydın parçaları. Bölümler kapalı başlar; bir bölümden sonraki `.open()` onu açar. `.single()` aynı anda tek bir bölümü açık tutar ve o zaman yalnızca açık işaretlenen ilk bölüm açık başlar. Bir accordion en fazla 50 bölüm tutar (`MAX_ACCORDION_SECTIONS`).

```rust
Accordion::new()
    .section(t!("help.password"), Paragraph::new(t!("help.password_answer")))
    .open()
    .section(t!("help.invoices"), Paragraph::new(t!("help.invoices_answer")))
    .single()
```

@live accordion

Her bölüm yerel bir açılır bölümdür (`<details>` ve `<summary>`), başlık ve düğmelerden oluşan ARIA accordion deseni değil; çünkü yerel öğe script gerektirmez, kapalı içeriği okuma sırasının dışında tutar ve tarayıcının sayfada bul özelliğinin eşleşmeyi içeren bölümü açmasını sağlar. Tekli kipte bölümler bir `name` paylaşır ve bir başkası açıldığında tarayıcı açık olanı kapatır. `Tabs`'tan farklı olarak, kapalı bir bölümün içeriği baştan beri sayfadadır.

| Tuş | Ne yapar |
|---|---|
| `Tab` | Sonraki bölüm başlığına geçer |
| `Enter` ya da `Space` | Bölümü açar ya da kapatır; odak başlığında kalır |

## Split

`Split::new(start, end)` aralarında bir tutamaç olan iki düğüm gösterir. `.ratio(percent)` başlangıç boyutunu belirler. Tutamaç işaretçiyle veya ok tuşlarıyla hareket eder.

## Dock

`Dock`, paneller bir merkez alanın çevresine yerleştirir: kullanıcıların taşıdığı, yeniden boyutlandırdığı, kapattığı ve geri getirdiği sol, sağ, alt ve merkez sekmeler. Her kullanıcının yerleşimi ekran başına kaydedilir. Bkz. [Shell ve komutlar](shell.md).

## Surface

`Surface::new(label, width, height)`, kişilerin öğeleri serbestçe yerleştirdiği sabit boyutlu bir alandır, örneğin bir kat planındaki masalar. `.item(Draggable::new(id, node).at(x, y).size(w, h))` bir öğe ekler, `.axis(Axis::X)` veya `Axis::Y` onu tek bir çizgide tutar, `.snap(step)` her konumu bir ızgaraya oturtur. `.on_move(action)` her taşımayı kaydeder: sunucu kimliğin bu alana ait olduğunu denetleyip konumu sınırların içinde, ızgarada ve eksende tuttuktan sonra eylem, öğenin kimliği ile yeni `x` ve `y` değerlerini taşıyan bir `SurfaceMove` alır.

`.resizable()` öğenin sağ alt köşesine bir tutamaç ekler; `.min_size(w, h)`, `.max_size(w, h)` ve `.aspect(w, h)` boyutu sınırlar ve öğeyi de yeniden boyutlandırılabilir yapar. Eylem bu durumda yeni `w` ve `h` değerlerini de alır; bunlar sınırların, alanın ve ızgaranın içinde tutulur. En boy oranı verildiğinde yükseklik genişliği izler.

```rust
Surface::new(t!("floor.plan"), 800, 480)
    .snap(20)
    .on_move(move_table)
    .item(
        Draggable::new("t1", Paragraph::new(t!("floor.table")))
            .at(40, 60)
            .size(120, 80),
    )
```

Fare veya kalem bir öğeyi sürükler; dokunmatik ekranda kısa bir basılı tutmadan sonra sürüklenir. Tek bir işaretçiyle, bir tıklama öğeyi seçer ve alana yapılan bir tıklama onu oraya koyar. Köşe tutamacına yapılan bir tıklama boyutu seçer, alana yapılan bir tıklama köşeyi oraya koyar.

| Tuş | Ne yapar |
|---|---|
| Ok tuşları | Odaktaki öğeyi bir ızgara adımı taşır (ızgara yoksa 10 piksel) |
| `Shift` + ok tuşları | Boyutlandırılabilir öğeyi bir ızgara adımı büyütür veya küçültür |
| `Escape` | Seçilen öğeyi bırakır veya sürüklenen öğeyi geri koyar |
