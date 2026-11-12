# الدردشة

مكوّنات للمحادثات: رسائل المحادثة، والمربع الذي يكتب فيه المستخدم، والإشارة إلى أن أحدًا يكتب الآن. يُرسم كل منها على الخادم، ولا يُحمّل ورقة أنماطه ونصه البرمجي إلا في الصفحات التي تستخدمه.

## MessageThread

يعرض `MessageThread` رسائل محادثة واحدة على هيئة فقاعات دردشة: رسائل المستخدم نفسه في جهة النهاية، ورسائل الآخرين في جهة البداية (معكوسة في اللغات التي تُكتب من اليمين إلى اليسار)، وتُجمع الرسائل المتتالية لمرسل واحد خلال خمس دقائق تحت اسم واحد، مع عنوان لكل يوم بحسب المنطقة الزمنية للمستخدم، ووقت كل رسالة، وحالة التسليم على رسائل المستخدم نفسه.

```rust
#[action(permission = "chat.read")]
pub async fn conversation_messages(q: ThreadQuery, store: Chats, ctx: Ctx) -> Result<Page<Msg>> {
    store.check_member(&q.conversation, &ctx)?;
    Ok(q.query.apply(store.messages(&q.conversation)))
}

MessageThread::of(conversation_messages)
    .conversation(params.id.to_string())
    .author(Msg::author)
    .name(Msg::author_name)
    .text(Msg::body)
    .when(Msg::sent_at)
    .status(Msg::status)
    .attachments(Msg::files, CHAT_FILES)
    .live()
```

يأخذ action السجل `ThreadQuery`: المحادثة، التي يأخذها الخادم من المكوّن (ولا يرسلها المتصفح أبدًا)، و`DataQuery` مرتبًا من الأحدث إلى الأقدم. تحقّق داخل action من أن المستخدم مشارك في المحادثة. `.author` هو معرّف المستخدم المرسل، ويُقارن بالمستخدم المسجّل دخوله للتمييز بين رسائله ورسائل الآخرين؛ و`.name` هو الاسم المعروض. يحتاج النموذج إلى `#[key]`، حتى تظهر الرسالة التي تُحمّل مرتين مرة واحدة فقط.

تُرسم أول 50 رسالة (`THREAD_PAGE`) مع الشاشة. ويؤدي التمرير إلى الأعلى إلى تحميل الصفحة الأقدم التالية عبر `POST /_zeyn/grid`، مع التحقق في كل مرة من صلاحية الوصول إلى الشاشة وإذن action، وتبقى الرسالة التي ينظر إليها القارئ في مكانها. لا يبقى في الصفحة أكثر من 200 رسالة: المضي أبعد يُسقط الطرف البعيد، الذي يُحمّل من جديد حين يعود القارئ، لذا تكلّف محادثة من آلاف الرسائل الذاكرة نفسها التي تكلّفها محادثة قصيرة. ويجري التمرير عبر 5,000 رسالة بمعدل إطارات الشاشة مع نحو 2,000 عنصر في الصفحة.

لـ `Delivery` الحالات `Sending` و`Sent` و`Delivered` و`Read` (إشعار القراءة) و`Failed`، وتُعرض كل منها بأيقونة ونص لقارئات الشاشة. يعرض `.attachments(field, rules)` ملفات الرسالة (حقل `Vec<File>` وقواعد مخزن مسجّل بـ `Feature::files`): الصور في مربع ثابت يُحمّل كسولًا فوق عنصر نائب، والملفات الأخرى روابط تنزيل تمر عبر إذن القراءة في المخزن.

تُجلب الرسائل الجديدة حين يُذكر أحد topics السلسلة، وهو `"chat"` ما لم يحدد `.topic(..)` غير ذلك: بعد أن يرسل `Composer`، وبعد أن ينتهي action بـ `ui.refresh("chat")`، أو مع `.live()` وميزة `realtime` حين يُنفَّذ `socket.notify(&ctx, "chat")` في أي مكان. وتُعلن رسائل الآخرين في منطقة حية مهذبة (polite).

### ردود الذكاء الاصطناعي

```rust
#[action(permission = "chat.ai")]
pub async fn ask_model(r: ReplyRequest, store: Chats) -> Result<Reply> {
    let id = store.reserve_id();
    let (tx, reply) = Reply::channel(id.to_string());
    tokio::spawn(async move {
        let mut text = String::new();
        for piece in my_model_stream(&r.text).await {
            text.push_str(&piece);
            if !tx.send(piece).await {
                break;
            }
        }
        store.save_ai_answer(id, &r.conversation, text);
    });
    Ok(reply)
}

MessageThread::of(conversation_messages)
    .conversation(id)
    .ai(Msg::from_ai)
    .reply(ask_model)
```

مع `.reply(action)`، تبدأ الرسالة المرسلة بـ `Composer` من المحادثة نفسها ردًا من الذكاء الاصطناعي: يرسل المتصفح النص إلى `POST /_zeyn/stream` (CSRF، وحد الكتابة، وصلاحية الوصول إلى الشاشة، وإذن action، والمعترضات)، ويعود الرد متدفقًا على هيئة أسطر JSON بصيغة المساعد المدمج. تمر الأجزاء عبر قناة محدودة (`REPLY_BUFFER`)؛ ويتوقف الخادم بعد `MAX_REPLY_CHARS` (32,000) حرف أو `REPLY_SECONDS` (120) ثانية، ويُنهي زر الإيقاف الرد مبكرًا. يعيد `ReplySender::send` القيمة `false` حين يغادر القارئ، وهذه إشارة المولّد إلى التوقف.

يُعرض الرد على هيئة مجموعة فرعية صغيرة من Markdown مبنية من عقد نصية فقط، فلا يُحلَّل أي شيء يكتبه النموذج على أنه HTML أبدًا. لا تُرسم الرسائل السابقة من جديد أبدًا، بل تُرسم الفقرة الجاري كتابتها فقط: رد من 2,000 جزء يكلّف لكل إطار في نهايته ما يكلّفه في بدايته، ولا يتحرك شيء مما فوقه. تقرأ المنطقة الحية الرد مرة واحدة حين يكتمل، ولا تقرأ كل جزء أبدًا. وتُحمّل الرسائل الموسومة بـ `.ai(field)` من السجل كـ Markdown آمن أيضًا (مع ميزة `markdown`؛ وكنص عادي من دونها).

مفتاح الرد (`Reply::channel(key)`) هو المفتاح الذي ستحمله الرسالة المخزنة، لذا يعرض السجل الإجابة المخزنة في مكانها بدلًا من نسخة ثانية.

## Composer

`Composer` هو المربع الذي يكتب فيه المستخدم الرسائل: منطقة نص تكبر مع نصها حتى ارتفاع أقصى، وزر إرسال، وعند الطلب مرفقات وإشارات إلى أشخاص ومسودة محفوظة.

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, socket: Socket, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    socket.notify(&ctx, "chat");
    Ok(())
}

pub const CHAT_DRAFTS: ClientPref<Draft> =
    ClientPref::draft("chat-draft", Duration::from_secs(7 * 86_400), Purpose::Preference);

Composer::of(send_message)
    .conversation(params.id.to_string())
    .attachments(CHAT_FILES)
    .mentions(search_people)
    .remember(CHAT_DRAFTS)
```

يأخذ action الإرسال `ChatSend`: المحادثة، التي يأخذها الخادم من المكوّن، والنص بعد إزالة المسافات من طرفيه، والملفات المرفقة، ومعرّفات الأشخاص المشار إليهم. يمر الإرسال عبر `POST /_zeyn/data` مع رمز CSRF، ويُحتسب ضمن حد الكتابة، ويحتاج إلى إذن action؛ ويُحذف من الصفحة كل composer لا يحق للمستخدم الإرسال به. يرفض الخادم الرسالة الفارغة، والرسالة الأطول من `.max_chars(n)` (4,000 ما لم يُحدَّد)، والملفات التي تزيد على ما تسمح به قواعد المخزن، ومعرّفات الملفات ذات الشكل الخاطئ، وأكثر من 20 إشارة. وتعامل مع الباقي أيضًا على أنه غير موثوق: تحقّق من أن المستخدم مشارك في المحادثة ويجوز له ذكر الأشخاص المشار إليهم.

`SendKey::Enter`، وهو الافتراضي، يرسل بـ Enter ويبدأ سطرًا جديدًا بـ Shift+Enter؛ و`.send_key(SendKey::CtrlEnter)` يرسل بـ Ctrl+Enter (Cmd+Enter على Mac) ويترك Enter للأسطر الجديدة. يكون زر الإرسال معطلًا ما دامت الرسالة فارغة أو طويلة جدًا، ويظهر عدّاد قرب الحد. بعد الإرسال يذكر composer الـ topics الخاصة به (`"chat"` ما لم يحدد `.topic(..)` غير ذلك)، فيعرض `MessageThread` للمحادثة نفسها الرسالة فورًا، ويبث رد الذكاء الاصطناعي متدفقًا حين يكون لديه `.reply(..)`.

يضيف `.attachments(rules)` حقل الرفع لمخزن مسجّل بـ `Feature::files`: تُرفع الملفات واحدًا تلو الآخر إلى `POST /_zeyn/files/{store}` مع شريط تقدم، بموجب إذن الكتابة في المخزن وفحوص النوع، وتذهب معرّفاتها مع الرسالة.

يبحث `.mentions(action)` عن الأشخاص بعد `@` باستخدام action بحث (يدخل `LookupQuery`، وتخرج صفحة من `LookupItem`). يبدأ البحث بعد 200 ms من آخر ضغطة مفتاح، ويلغي البحث الأحدث البحث الأقدم، ويطلب الخادم `MAX_MENTIONS` (8) أشخاص على الأكثر ولا يعيد أكثر من ذلك. القائمة listbox تُدار من منطقة النص: الأسهم للتنقل، وEnter أو Tab للاختيار، وEscape للإغلاق.

يحفظ `.remember(key)` النص غير المرسل لكل محادثة في مخزن تفضيلات العميل تحت مفتاح `Draft` مسجّل بـ `App::client_prefs`، إلى أن يُرسل. لا تتجاوز المسودة `MAX_DRAFT_CHARS` (4,000) حرف، ولا يُحفظ أكثر من `MAX_DRAFTS` (20) محادثة، ويُحذف الأقدم أولًا. لا يُخزَّن معرّف المحادثة إلا على هيئة hash. ومع `Purpose::Preference` تبقى المسودات طوال الجلسة إلى أن يوافق المستخدم على أكثر من ذلك.

## TypingIndicator

يعرض `TypingIndicator` من غير المستخدم يكتب الآن في المحادثة: "Ada تكتب"، "Ada وGrace يكتبان"، "3 أشخاص يكتبون"، في منطقة حية مهذبة (polite).

```rust
let live = ZeynSocket::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

يعمل عبر قناة الزمن الحقيقي (ميزة `realtime`، انظر `realtime.md`) على topic مُعلن بـ `ZeynSocket::topic`. بينما يكتب المستخدم في `Composer` للمحادثة نفسها، يرسل المتصفح إشارة صغيرة عبر المقبس الحي للصفحة مرة كل 3 ثوانٍ على الأكثر؛ ويُسقط الخادم الإشارات الأسرع لكل مستخدم ويمرر الإشارة إلى المشتركين الآخرين في الـ topic داخل المستأجر، ولا يعيدها أبدًا إلى المستخدم نفسه. تحمل الإشارة معرّف المحادثة ومفتاحًا للمستخدم لا يستطيع إنشاءه إلا هذا الخادم، ولا تحمل أبدًا معرّف المستخدم أو أي نص. يظهر الاسم مدة 6 ثوانٍ بعد آخر إشارة؛ ويظهر الأشخاص الذين لم يُعطَوا بـ `.person(user, name)` باسم "شخص ما".

من دون ميزة `realtime`، أو حين لا تكون للتطبيق قناة زمن حقيقي، لا يرسم المؤشر شيئًا، ولا يُحمّل نصه البرمجي أبدًا، ولا يُرسل شيء. تصل الإشارات إلى كل مشترك في الـ topic داخل المستأجر، فيكون معرّف المحادثة مرئيًا لهم: استخدم معرّفات معتمة، أو topic لكل جمهور.
