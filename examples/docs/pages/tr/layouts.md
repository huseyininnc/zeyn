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

`Card`, isteğe bağlı başlığı olan düz bir yüzeydir. `Panel`, isteğe bağlı araç çubuğu olan başlıklı bir bölümdür; `.collapsible()` kullanıcının onu katlamasına izin verir, `.collapsed()` katlı başlatır.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` ile `.tab(title, node)`. Ok tuşları, ARIA tabs deseninin tarif ettiği gibi sekmeler arasında gezinir. Kapalı bir sekmenin içeriği, sekme ilk açılana kadar render edilmez; böylece çok sekmeli bir ekran hafif kalır.

## Split

`Split::new(start, end)` aralarında bir tutamaç olan iki düğüm gösterir. `.ratio(percent)` başlangıç boyutunu belirler. Tutamaç işaretçiyle veya ok tuşlarıyla hareket eder.

## Dock

`Dock`, paneller bir merkez alanın çevresine yerleştirir: kullanıcıların taşıdığı, yeniden boyutlandırdığı, kapattığı ve geri getirdiği sol, sağ, alt ve merkez sekmeler. Her kullanıcının yerleşimi ekran başına kaydedilir. Bkz. [Shell ve komutlar](shell.md).
