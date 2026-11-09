# جزر WASM

يولّد Zeyn الصفحات على الخادم. وبعض الأسطح أكثر تفاعلاً من أن يناسبها ذلك: محرر لوحة رسم (canvas)، أو مخطط، أو رسم بياني ثقيل. **الجزيرة (island)** مكوّن Rust مصرَّف إلى WebAssembly يضعه الخادم في صفحة تُولَّد على الخادم فيما عداه. وكل ما حوله يبقى HTML عادياً.

الجزر خلف ميزة Cargo المسماة `islands` في `zeyn`. التطبيقات التي لا تفعّلها لا تصرّف شيئاً من هذا، والصفحات التي بلا جزيرة لا تدفع شيئاً حتى مع تفعيلها (انظر الأداء أدناه).

## 1. الأجزاء

| الجزء | أين | الدور |
|---|---|---|
| `zeyn::ui::Island` | الخادم | عقدة نائبة: الخصائص (props) والمحتوى البديل والإجراءات المربوطة |
| `App::islands(dir)` | الخادم | يقدّم الملفات المبنية والمحمِّل |
| `zeyn-island` | مشترك والمتصفح | `Remote` (عقد الإجراء القابل للاستدعاء) و`Island::call` (جهة العميل) |
| حزمة (crate) مشتركة | كلاهما | الخصائص والرسائل والاستدعاءات البعيدة أنواع serde عادية |
| حزمة جزيرة | المتصفح | `cdylib` يصدّر `mount(root)` واختيارياً `unmount(root)` |
| `zeyn build-islands` | CLI | يبني حزم الجزر إلى ملفات بتجزئة وملف manifest |

تخطيط نموذجي، كما في `examples/showcase`:

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. الأنواع المشتركة

تعتمد الحزمة المشتركة على `serde` وعلى `zeyn-island` بلا ميزات، فلا يصرّف الخادم أي شيفرة للمتصفح. يسمّي الاستدعاء البعيد (remote) إجراءً يجوز للجزيرة استدعاؤه ويثبّت أنواعه:

```rust
#[derive(Serialize, Deserialize)]
pub struct Count { pub value: i64 }

pub struct SaveCount;

impl zeyn_island::Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
```

## 3. جهة الخادم

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

لا يُصرَّف `action::<R, A>` إلا عندما يكون مدخل الإجراء `R::Input` وخرجه `Result<R::Output>`، فلا يمكن أن يتباعد الطرفان. ولا تستطيع الجزيرة استدعاء إلا الإجراءات التي تربطها عقدتها.

يقبل `fallback` أي عقد: ملخصاً نصياً أو شبكة للقراءة فقط. ويظهر قبل تحميل الجزيرة، وعند تعطيل JavaScript، وعند فشل التحميل. وتعمل شبكاته ومكوناته الأخرى كالمعتاد.

التحميل: `.on_interaction()` ينتظر أول مؤشر أو لمس أو تركيز على العقدة النائبة، و`.on_idle()` يحمّل عندما يكون المتصفح خاملاً، والافتراضي يحمّل عندما تقترب العقدة النائبة إلى مسافة 200 بكسل من نافذة العرض. ولا يحجب التحميل الصفحة أبداً.

سجّل مجلد الخرج مرة واحدة: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`. وبدون manifest هناك يسجل الخادم تحذيراً وتعرض الجزر محتواها البديل.

## 4. جهة الجزيرة

```rust
#[wasm_bindgen]
pub fn mount(root: web_sys::Element) -> Result<(), JsValue> {
    let island = zeyn_island::Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    // build DOM under root, attach listeners
    zeyn_island::spawn(async move {
        match island.call::<SaveCount>(&Count { value: 3 }).await {
            Ok(saved) => { /* ... */ }
            Err(e) => { /* e.message is localized by the server */ }
        }
    });
    Ok(())
}
```

يرسل `Island::call` طلب `POST /_zeyn/data` مع شاشة الجزيرة ومعاملاتها ومسار عقدتها، واسم الاستدعاء البعيد بوصفه `key`، والمدخل. ويرسل رمز CSRF للصفحة في `x-zeyn-csrf` وملف تعريف ارتباط الجلسة. ويفحص الخادم، كما في كل مكوّن: CSRF، ووصول الشاشة، وحراس الميزة، وصلاحية الإجراء، ثم يشغّل المعترضات والإجراء مع حد الوقت. لا يوجد مدخل آخر: لا تستطيع الجزيرة الوصول إلى إجراء لا تربطه عقدتها، ولا شيء في الجزر يتجاوز أي فحص.

تعود الأخطاء بصيغة `CallError { status, code, message }`. وتكون `status` هي `0` للإخفاقات المحلية (`network` و`props` و`input` و`answer`).

صدّر `unmount(root)` عندما تحتفظ الجزيرة بمؤقتات أو إطارات حركة أو مقابس. يستدعيها المحمِّل عندما تغادر العقدة النائبة المستند (تنقل، أو نافذة أُغلقت).

الترجمات: مرّر النصوص التي تعرضها الجزيرة في خصائصها، مترجمة أصلاً من الخادم، فلا تحمل الجزيرة فهرساً.

## 5. البناء

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

لكل حزمة يشغّل:

1. `cargo build --lib --release --target wasm32-unknown-unknown` مع إعدادات الحجم الممررة بـ `--config` (`opt-level = "z"` و`lto = true` و`codegen-units = 1` و`panic = "abort"`، بلا معلومات تنقيح)، فلا يُمَسّ ملف release الخاص بالتطبيق؛
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section` (يجب أن يطابق إصدار `wasm-bindgen-cli` حزمة `wasm-bindgen`، أي `0.2.130`)؛
3. `wasm-opt -Oz` عندما يكون binaryen في `PATH`؛
4. يكتب `{name}.{hash}.js` و`{name}_bg.{hash}.wasm` (تجزئة FNV بطول 64 بت للمحتوى) ويحدّث `manifest.json`.

يفعل `zeyn dev` الشيء نفسه قبل كل بناء للتطبيق عندما يكون فيه مجلد `islands/`، ولا يراقب `dist/`.

لا يقدّم الخادم إلا أسماء الملفات المدرجة في manifest، عند `/_zeyn/islands/{file}`، مع `application/wasm` أو `text/javascript` و`Cache-Control: public, max-age=31536000, immutable`. وتُقرأ الملفات من القرص عند كل طلب، فلا تكلف الخادم ذاكرة.

## 6. سياسة أمان المحتوى

يحتاج تصريف WebAssembly إلى `'wasm-unsafe-eval'` في `script-src`. يضيفها Zeyn **فقط** إلى الصفحات التي تحتوي شاشتها جزيرة؛ وكل صفحة أخرى تحتفظ بالسياسة الصارمة الموصوفة في `docs/runtime-protocol.md`. `'wasm-unsafe-eval'` تسمح بـ `WebAssembly.compile` و`instantiate` ولا شيء غيرهما: يبقى `eval` و`new Function` والسكربتات المضمنة محجوبة، وتأتي الوحدات من أصل التطبيق فقط.

السياسة تخص المستند ولا يمكن تخفيفها لاحقاً. لذلك عندما يحمّل `nav.js` شاشة فيها جزيرة (تحمل الإجابة الجزئية `x-zeyn-islands: 1`) في مستند حُمِّل بدونها، فإنه يرجع إلى تحميل صفحة كاملة. وما إن يحصل المستند على سياسة الجزر، يبقى التنقل بين شاشات الجزر والشاشات العادية في مكانه.

يجد الخادم الجزر بالخاصية `data-zeyn-island="`؛ ولا تستطيع بيانات المستخدم المهرَّبة أن تنتج علامة الاقتباس أبداً، فلا يستطيع نص يكتبه المستخدم أن يفعّل السياسة.

## 7. أسطح Canvas

`zeyn::ui::Canvas::new(name, description)` جزيرة للرسم. الوصف مطلوب: لوحة الرسم غير مرئية للتقنيات المساعدة، لذلك يجب أن تقول الصفحة ما تعرضه وما المفاتيح التي تشغّلها. يولّدها الخادم فقرة فوق الجزيرة ويوجّه `aria-describedby` للعقدة النائبة إليها.

في جهة الجزيرة، توفّر `zeyn-island` مع ميزة `canvas` النوع `canvas::Surface`:

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| الاهتمام | ما يفعله السطح |
|---|---|
| الرسوميات | `Prefer::Gl` يجرّب WebGL 2 ثم يرجع إلى 2D؛ و`webgpu_available()` تخبر هل `navigator.gpu` موجود للمشاهد التي تُعدّ WebGPU بنفسها على `surface.canvas()` |
| الدقة العالية | يتبع مخزن الدعم `devicePixelRatio` وحجم العنصر (`ResizeObserver`)؛ والرسم ثنائي الأبعاد مضروب مسبقاً بالمقياس، فتعمل المشاهد بوحدات CSS بكسل |
| الإدخال | ضغط المؤشر وحركته ورفعه (مع التقاط المؤشر) والعجلة (تُحوَّل فروق الأسطر والصفحات إلى بكسلات) والمفاتيح، بصيغة قيم `Input` بوحدات CSS بكسل |
| الإطارات | `requestAnimationFrame` فقط بعد إدخال استخدمته المشهد، أو `update`، أو تغيير حجم، أو ما دامت `Scene::animating` تعيد `true`. الصورة الثابتة لا تكلف أي معالج |
| الافتراضيات | الإدخال الذي يستخدمه المشهد يُمنع سلوكه الافتراضي (الأسهم لا تمرّر الصفحة)؛ أعد `false` لمفتاح Tab حتى يستطيع التركيز المغادرة |
| التنظيف | `destroy()` يزيل المستمعين والمراقب ولوحة الرسم؛ استدعِها من `unmount` |

تحصل لوحة الرسم على `tabindex="0"` و`role="application"` والتسمية و`aria-describedby`.

نمط الإتاحة، كما في محرر المخططات في showcase (`examples/showcase/islands/diagram`):

- **لوحة المفاتيح على لوحة الرسم:** `]` و`[` تحددان الأشكال، والأسهم تحركها، وDelete تزيل، و`+` و`-` و`0` للتكبير، وEscape تمسح التحديد. لا تعمل المفاتيح إلا بينما تملك لوحة الرسم التركيز.
- **عناصر تحكم عادية:** شريط أدوات من أزرار يفعل الشيء نفسه دون لوحة الرسم.
- **ملخص نصي:** قائمة تحت لوحة الرسم تصف كل شكل وتتبع كل تغيير؛ وسطر `role="status"` يعلن التحديد ونتائج الحفظ.
- **المحتوى البديل:** يولّد الخادم الملخص نفسه، فيقرأ المستخدمون الذين بلا WebAssembly المخطط أيضاً.

يحفظ المحرر عبر `SaveDiagram`، ويقصّ الخادم ما يتلقاه (`Diagram::clamped`: 200 شكل على الأكثر، ومعرّفات فريدة، وتسميات من 40 حرفاً، وإحداثيات محدودة) لأن الجزيرة لا يُوثق بها أبداً.

### 7.1 مفتش الخصائص

يحرّر `PropertyInspector::island(name)` الكائن المحدد. ترسل الجزيرة `zeyn:inspect` مع `detail: { title, schema, values }` من أي عنصر فيها، ويرد المفتش على كل تغيير مؤكد بـ`zeyn:property` (`detail: { name, value }`) على جذر الجزيرة بعد التحقق منه. وتظل الجزيرة تتحقق مما تحفظه، ويتحقق الخادم من الحفظ بـ`FormSchema::check`.

## 8. الأداء

| ماذا | الرقم |
|---|---|
| HTML الصفحة العادية والطلبات والسياسة | دون تغيير: لا meta ولا محمِّل ولا طلب إضافي |
| `nav.js` | +281 بايت (بديل تحميل الصفحة الكاملة) |
| محمِّل `/_zeyn/islands.js`، لصفحات الجزر فقط | 3.9 كيلوبايت، و1.7 كيلوبايت مضغوطة بـ gzip |
| `.wasm` لجزيرة العداد (بلا wasm-opt) | 127.6 كيلوبايت، و52.8 كيلوبايت gzip |
| `.js` الرابط لجزيرة العداد | 18.0 كيلوبايت، و4.2 كيلوبايت gzip |
| `.wasm` لمحرر المخططات (سطح canvas، بلا wasm-opt) | 169.4 كيلوبايت، و67.8 كيلوبايت gzip |
| محرر المخططات مع التحرير الحي (يضيف `yrs`، انظر `docs/realtime.md`) | 408.4 كيلوبايت، و144.3 كيلوبايت gzip |
| `.js` الرابط لمحرر المخططات | 30.3 كيلوبايت، و6.1 كيلوبايت gzip |
| ذاكرة الخادم لكل جزيرة | لا شيء؛ تتدفق الملفات من القرص، وتُولَّد الخصائص عند كل طلب |

معظم حجم العداد يعود إلى `serde_json` وشيفرة التنسيق في `core`. الجزر التي تتشارك هذه تدفع ثمنها مرة لكل منها؛ و`wasm-opt` يخفض عادة 10 إلى 20 بالمئة أخرى.

## 9. الحدود

- جزيرة داخل نافذة (`open_window`) تحتاج أن تحمل صفحة الفتح سياسة الجزر؛ ضع النوافذ التي فيها جزر في صفحات جزر أو افتحها شاشات.
- الإغلاقات (closures) الملحقة بـ `Closure::forget` تعيش ما دامت الصفحة؛ حررها في `unmount` عندما يمكن تركيب جزيرة مرات كثيرة.
- لا تولّد الجزر أي شيء على الخادم بنفسها؛ والمحتوى البديل هو ما تحصل عليه محركات البحث وقارئات الشاشة قبل التحميل والمستخدمون بلا JavaScript. اجعله ذا معنى.
