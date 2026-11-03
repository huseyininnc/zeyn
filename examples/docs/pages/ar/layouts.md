# التخطيطات

مكونات التخطيط (Layout) تضع مكونات أخرى في أماكنها. لا تحمل بيانات ولا تضيف طلبات؛ والشاشة المبنية منها استجابة HTML واحدة.

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

عقد تُوضع واحدة تحت الأخرى. يضبط `.gap(units)` المسافة بينها بوحدات التباعد (4 بكسل عند الكثافة العادية)، من 0 إلى 12.

## Row

أعمدة جنباً إلى جنب. يأخذ كل `.col(node)` حصة من العرض؛ ويجعل `.weight(n)` بعد العمود هذا العمود أعرض. على الشاشات الضيقة تتراص الأعمدة فوق بعضها. يضغط `.compact()` العناصر الصغيرة مثل الشارات.

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card وPanel

`Card` سطح بسيط بعنوان اختياري. `Panel` قسم له عنوان وشريط أدوات اختياري؛ يتيح `.collapsible()` للمستخدم طيّه، ويجعله `.collapsed()` يبدأ مطوياً.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` مع `.tab(title, node)`. تنقل مفاتيح الأسهم بين علامات التبويب، كما يصف نمط ARIA للتبويبات. لا يُعرض محتوى التبويب المغلق حتى يُفتح أول مرة، لذلك تبقى الشاشة ذات التبويبات الكثيرة خفيفة.

## Split

يعرض `Split::new(start, end)` عقدتين بينهما مقبض. يضبط `.ratio(percent)` حجم البداية. يتحرك المقبض بالمؤشر أو بمفاتيح الأسهم.

## Dock

ينظّم `Dock` اللوحات حول منطقة مركزية: لوحات يسار ويمين وأسفل وتبويبات مركزية يحركها المستخدمون ويغيرون حجمها ويغلقونها ويعيدونها. يُحفظ تخطيط كل مستخدم لكل شاشة. راجع [Shell and commands](shell.md).
