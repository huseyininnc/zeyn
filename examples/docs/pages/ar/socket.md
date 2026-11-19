# ZeynSocket

يحمل اتصال حي واحد لكل صفحة كل ما يرسله الخادم: إشعارات المواضيع التي تحدّث الشاشات، والأحداث ذات الأنواع المحددة الموجهة إلى مستخدم أو مجموعة أو tenant، وأفعال action التي يستدعيها المتصفح من دون طلب صفحة. فعّله بميزة `realtime`:

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
App::new("Shop")
    .socket(
        ZeynSocket::new()
            .topic("orders", "sales.order.read")
            .event::<OrderShipped>("sales.order.read")
            .event::<MessageSent>("chat.read"),
    )
```

يسجّل `App::socket` أيضاً الخدمة `Socket`، لذا تستطيع أفعال action أن تأخذ `socket: Socket`. بروتوكول النقل والحدود وأسبابها موجودة في `docs/zeynsocket.md`.

## المواضيع: شاشات تتحدث

لا يحمل الموضوع أي بيانات. إنه يخبر الشاشات التي تعرض بيانات معينة بأن تعيد تحميلها، عبر نقاط النهاية العادية وفحوص الصلاحيات الخاصة بها.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

صرّح بكل موضوع عبر `ZeynSocket::topic` مع الصلاحية التي يحتاجها المشترك. العقد المعلَّمة بـ `.listens("orders")` تتحدث أيضاً.

## الأحداث: بيانات لبعض المستخدمين

الحدث نوع `Serialize` له اسم ثابت. يجب أن يملك كل مستلم الصلاحية التي صُرّح بالحدث معها؛ والمجموعة لا تمنح هذه الصلاحية أبداً.

```rust
#[derive(Serialize, Event)]
#[event(name = "chat.message")]
pub struct MessageSent {
    pub room: String,
    pub text: String,
}

socket.to_user(&ctx, "ayse").emit(&notice)?;          // every open page of one user
socket.to_group(&ctx, format!("room:{id}")).emit(&m)?; // members of a group
socket.to_tenant(&ctx).emit(&notice)?;                 // the whole tenant
socket.to_user_id(Some("acme"), "ayse").emit(&n)?;     // from a job, without a request
```

يبقى كل شيء داخل tenant واحد. لا تحمل الأحداث أبداً قيماً مقنّعة: أرسل id ودع المستلم يحمّل السجل عبر action، الذي يطبّق الإقناع كالمعتاد. الحدث الأكبر من `Limits::event_bytes` (16 KB) أو الذي لم يُصرَّح به يعيد خطأً ولا يرسل شيئاً.

## المجموعات

ينضم الاتصال إلى مجموعة فقط داخل action مستدعى عبر socket. في ذلك action يتحقق التطبيق من أن المستخدم يحق له أن يكون في المجموعة.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

يوجد `Conn` فقط في الاستدعاءات عبر socket؛ والـ action نفسه إذا استُدعي عبر HTTP يجيب بـ `422`. يكون الاتصال في `Limits::groups` (32) مجموعة على الأكثر.

## شاشات من دون JavaScript

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

يحدّث `OnEvent::Refresh` العقدة في مكانها عندما يصل الحدث إلى هذا المستخدم؛ ويعرض `OnEvent::Toast` إشعاراً منبثقاً (toast). لا يصل إلا ما يحق للمستخدم استلامه من أحداث.

## شيفرة المتصفح

تستخدم الجزر (islands) والسكربتات الخاصة `zeyn.socket`:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

يمكن استدعاء أفعال action المنشورة عبر `Feature::api`. يمر كل استدعاء بالفحوص نفسها التي يمر بها استدعاء API: الصلاحية، وguards، وinterceptors (نوع الاستدعاء `Live`)، والتحقق، والتدقيق، وحد الكتابة الخاص بالتطبيق لأفعال action التي تغيّر البيانات.

## اتصال لا ينقطع

يعيد المتصفح الاتصال بفترات انتظار عشوائية تطول تدريجياً، وفوراً عندما يعود إلى الشبكة. بعد الانقطاع يحتفظ الخادم بمواضيع الاتصال ومجموعاته وآخر رسائله مدة `Limits::detached` (60 ثانية) ويعيد إرسال ما فات الصفحة؛ وعندما لا يكون ذلك ممكناً تعيد الصفحة تحميل أجزائها الحية. وحيث تحجب الوكلاء (proxies) WebSocket، تنتقل الصفحة من تلقاء نفسها إلى Server-Sent Events للاستقبال و`POST` للإرسال، بالرسائل نفسها.

## عدة خوادم

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

تسلّم كل نسخة إلى اتصالاتها ما تنشره النسخ الأخرى، لذا لا حاجة إلى جلسات ثابتة (sticky sessions). وما دام Redis متوقفاً تبقى الإرسالات داخل النسخة.

## الاختبارات

تضيف ميزة `socket-client` الأداة `TestSocket`، وهي عميل يتحدث مع socket كما يفعل المتصفح:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## الحدود

| الحد | القيمة الافتراضية |
|---|---|
| sockets المفتوحة لكل مستخدم | 8 |
| الرسائل المنتظرة لكل اتصال | 32 |
| الاستدعاءات لكل اتصال في الثانية | 20 |
| الإطارات لكل اتصال في الثانية | 100 |
| أكبر إطار | 64 KB |
| أكبر حدث | 16 KB |
| مخزن إعادة الإرسال لكل اتصال | 64 رسالة، 64 KB |
| مدة الاحتفاظ بالاتصال المنقطع | 60 s |

يكلّف الاتصال الخامل نحو 16 KB، مقيساً مع تشغيل الخادم والعميل في عملية واحدة.
