# التأثيرات والنوافذ

التأثيرات (effects) هي ما يفعله المتصفح بعد نقرة أو بعد إجراء ناجح: فتح نافذة، أو الانتقال إلى شاشة، أو إظهار إشعار (toast)، أو طلب تأكيد، أو إغلاق نافذة وتحديث الشاشة التي تحتها. تُعلن في Rust ويشغّلها وقت التشغيل (runtime)، ولذلك لا تُكتب أي JavaScript.

## التأثيرات

| التأثير | يُبنى بـ | النتيجة |
|---|---|---|
| فتح نافذة | `Window::open(screen, params)` | يفتح الشاشة في نافذة مشروطة (modal) |
| الانتقال | `Effect::navigate(screen, params)` | ينتقل إلى الشاشة في مكانه، كرابط |
| إشعار | `Effect::Toast { message }` | يعرض رسالة قصيرة |
| إشعار بنبرة | `Toast::success(message).action(label, effect).into()` | إشعار بنبرة (info أو success أو warning أو danger) مع زر اختياري يشغّل تأثيرًا آخر |
| تأكيد | `Effect::confirm(message, then)` | يشغّل `then` بعد أن يؤكد المستخدم |
| إغلاق وتحديث | `Effect::CloseAndRefresh` | يغلق النافذة ويعيد تحميل الشاشة التي فتحتها |

المعاملات (params) مكتوبة الأنواع: تأثير لشاشة بمعاملات خاطئة لا يُترجم.

يصل `Effect::navigate` إلى شاشة يحتوي مسارها على أجزاء (راجع [المعاملات في المسار](screens.md)) إلى صيغة المسار، مثل `/orders/42`؛ أما المعاملات غير الموجودة في المسار فتذهب في استعلام `params`.

## بعد إجراء

تأخذ النماذج وأزرار الإجراءات معالج `on_success`. يستقبل نتيجة الإجراء و`UiCtx` الذي يجمع التأثيرات:

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

في `UiCtx` الدوال `toast` و`notify` و`close_and_refresh` و`navigate` و`effect` لأي شيء آخر، مثل التأكيد:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## النوافذ

النافذة شاشة بلا مسار، تُبنى بـ `Screen::window(title)`. تتراكم النوافذ: يمكن لنافذة أن تفتح أخرى. يغلق Escape أو زر الإغلاق النافذة العليا ويعود التركيز إلى العنصر الذي فتحها. يضبط `.width(px)` عرضها.

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## من أين تأتي التأثيرات

- روابط الشبكات وفتح الصفوف: `.link(|p| Window::open(...))` و`.on_open(|p| ...)`.
- أزرار شريط الأدوات: `.opens(screen, params)`.
- قوائم السياق: `.item(label, effect)`.
- الأوامر: `Command::open` و`Command::navigate` و`Command::effect`.
- معالجات `on_success` في النماذج وأزرار الإجراءات.

التأثيرات لا تعيد تحميل الصفحة أبدًا. يحمّل الانتقال الشاشة الجديدة فقط؛ ويعيد التحديث تحميل الشاشة التي تحت النافذة فقط.

## الإشعارات (Toasts)

يعرض `ui.toast(text)` إشعار info. ويختار `ui.notify(Toast::success(text))` النبرة: `Toast::info` أو `success` أو `warning` أو `danger`. تضيف `.action(label, effect)` زرًا يشغّل تأثيرًا آخر، مثل فتح السجل الذي حُفظ:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

تبقى الإشعارات أربع ثوانٍ (وإشعارات danger ثماني)، ويتوقف الوقت ما دام المؤشر فوق الإشعار أو ما دام يحمل التركيز، حتى يمكن الوصول إلى أزراره. تُقرأ إشعارات info وsuccess وwarning بهدوء من منطقة حالة (status region)؛ أما إشعارات danger فتنبيهات (alerts) تُعلن فورًا. لكل إشعار زر إغلاق، وتُعطى النبرة أيضًا بشريط لونه ونصه، ولا يظهر أكثر من ثلاثة إشعارات في وقت واحد (`MAX_TOASTS`): الجديد يزيل الأقدم.

@live toasts
