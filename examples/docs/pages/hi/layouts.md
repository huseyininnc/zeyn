# लेआउट

लेआउट कंपोनेंट दूसरे कंपोनेंट को रखते हैं। वे कोई डेटा नहीं रखते और कोई अनुरोध नहीं जोड़ते; उनसे बनी Screen एक HTML उत्तर है।

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

नोड एक के नीचे एक। `.gap(units)` उनके बीच की जगह स्पेसिंग इकाइयों में (सामान्य घनत्व पर 4 px) तय करता है, 0 से 12।

## Row

कॉलम साथ-साथ। हर `.col(node)` चौड़ाई का एक हिस्सा लेता है; कॉलम के बाद `.weight(n)` उसे चौड़ा बनाता है। संकरी स्क्रीन पर कॉलम ऊपर-नीचे लग जाते हैं। `.compact()` बैज जैसी छोटी चीज़ें पास-पास रखता है।

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card और Panel

`Card` वैकल्पिक शीर्षक वाली सादी सतह है। `Panel` वैकल्पिक टूलबार वाला शीर्षक युक्त खंड है; `.collapsible()` उपयोगकर्ता को उसे समेटने देता है और `.collapsed()` उसे समेटे हुए शुरू करता है।

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`.tab(title, node)` के साथ `Tabs::new(label)`। तीर कुंजियाँ टैब के बीच चलती हैं, जैसा ARIA tabs पैटर्न बताता है। बंद टैब की सामग्री तब तक रेंडर नहीं होती जब तक टैब पहली बार नहीं खुलता, इसलिए कई टैब वाली Screen हल्की रहती है।

## Split

`Split::new(start, end)` दो नोड उनके बीच के हैंडल के साथ दिखाता है। `.ratio(percent)` शुरुआत का आकार तय करता है। हैंडल पॉइंटर या तीर कुंजियों से चलता है।

## Dock

`Dock` पैनलों को केंद्र क्षेत्र के चारों ओर व्यवस्थित करता है: बायाँ, दायाँ, नीचे और केंद्र के टैब जिन्हें उपयोगकर्ता हिलाते, उनका आकार बदलते, बंद करते और वापस लाते हैं। हर उपयोगकर्ता का लेआउट प्रति Screen सहेजा जाता है। देखें [शेल और कमांड](shell.md)।
