# ZeynSocket

हर पृष्ठ का एक लाइव कनेक्शन वह सब ले जाता है जो सर्वर भेजता है: topic सूचनाएँ जो Screen को रिफ़्रेश करती हैं, किसी उपयोगकर्ता, समूह या tenant के लिए टाइप किए गए event, और वे action जिन्हें ब्राउज़र पृष्ठ अनुरोध के बिना कॉल करता है। इसे `realtime` feature से चालू करें:

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

`App::socket` `Socket` सेवा भी पंजीकृत करता है, इसलिए action `socket: Socket` ले सकते हैं। वायर प्रोटोकॉल, सीमाएँ और उनके कारण `docs/zeynsocket.md` में हैं।

## Topic: Screen जो रिफ़्रेश होती हैं

Topic कोई डेटा नहीं ले जाता। यह कुछ डेटा दिखाने वाली Screen को बताता है कि वे उसे फिर से लोड करें, सामान्य एंडपॉइंट और उनकी अनुमति जाँच के ज़रिए।

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

हर topic को `ZeynSocket::topic` से घोषित करें, साथ में वह अनुमति जो सब्सक्राइबर के पास होनी चाहिए। `.listens("orders")` से चिह्नित नोड भी रिफ़्रेश होते हैं।

## Event: कुछ उपयोगकर्ताओं के लिए डेटा

Event एक `Serialize` टाइप है जिसका नाम स्थिर रहता है। हर प्राप्तकर्ता के पास वह अनुमति होनी चाहिए जिसके साथ event घोषित किया गया था; कोई समूह यह अनुमति कभी नहीं देता।

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

सब कुछ एक ही tenant के भीतर रहता है। Event कभी मास्क किए गए मान नहीं ले जाते: एक id भेजें और प्राप्तकर्ता को रिकॉर्ड किसी action के ज़रिए लोड करने दें, जो हमेशा की तरह मास्क करता है। `Limits::event_bytes` (16 KB) से बड़ा या कभी घोषित न किया गया event त्रुटि लौटाता है और कुछ नहीं भेजता।

## समूह

कोई कनेक्शन किसी समूह में केवल सॉकेट पर कॉल किए गए action के भीतर जुड़ता है। उसी action में ऐप जाँचता है कि उपयोगकर्ता उस समूह में रह सकता है या नहीं।

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` केवल सॉकेट पर होने वाली कॉल में मौजूद होता है; वही action HTTP पर कॉल करने पर `422` लौटाता है। एक कनेक्शन अधिकतम `Limits::groups` (32) समूहों में रहता है।

## JavaScript के बिना Screen

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` तब नोड को उसी जगह रिफ़्रेश करता है जब event इस उपयोगकर्ता तक पहुँचता है; `OnEvent::Toast` एक टोस्ट दिखाता है। केवल वही event आते हैं जिन्हें उपयोगकर्ता प्राप्त कर सकता है।

## ब्राउज़र कोड

Island और अपनी स्क्रिप्ट `zeyn.socket` का उपयोग करते हैं:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

`Feature::api` से प्रकाशित action कॉल किए जा सकते हैं। हर कॉल वही जाँचें चलाती है जो API कॉल चलाती है: अनुमति, guard, interceptor (कॉल प्रकार `Live`), सत्यापन, ऑडिट, और डेटा बदलने वाले action के लिए ऐप की लेखन सीमा।

## ऐसा कनेक्शन जो नहीं टूटता

ब्राउज़र बढ़ते हुए, यादृच्छिक अंतराल के साथ फिर से जुड़ता है, और ऑनलाइन लौटते ही तुरंत जुड़ता है। कनेक्शन टूटने के बाद सर्वर कनेक्शन के topic, समूह और अंतिम संदेश `Limits::detached` (60 सेकंड) तक रखता है और पृष्ठ से जो छूट गया उसे फिर से भेजता है; जब यह संभव नहीं होता, पृष्ठ अपने लाइव हिस्से फिर से लोड करता है। जहाँ प्रॉक्सी WebSocket को रोकते हैं, वहाँ पृष्ठ अपने आप नीचे की दिशा के लिए Server-Sent Events और ऊपर की दिशा के लिए `POST` पर चला जाता है, संदेश वही रहते हैं।

## कई सर्वर

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

हर इंस्टेंस दूसरों द्वारा प्रकाशित चीज़ें अपने कनेक्शनों तक पहुँचाता है, इसलिए sticky session की ज़रूरत नहीं होती। जब तक Redis बंद है, भेजे गए संदेश उसी इंस्टेंस पर रहते हैं।

## परीक्षण

`socket-client` feature `TestSocket` जोड़ता है, एक क्लाइंट जो ब्राउज़र की तरह सॉकेट से बात करता है:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## सीमाएँ

| सीमा | डिफ़ॉल्ट |
|---|---|
| प्रति उपयोगकर्ता खुले सॉकेट | 8 |
| प्रति कनेक्शन प्रतीक्षारत संदेश | 32 |
| प्रति कनेक्शन प्रति सेकंड कॉल | 20 |
| प्रति कनेक्शन प्रति सेकंड फ़्रेम | 100 |
| सबसे बड़ा फ़्रेम | 64 KB |
| सबसे बड़ा event | 16 KB |
| प्रति कनेक्शन रीप्ले बफ़र | 64 संदेश, 64 KB |
| टूटे कनेक्शन को रखने का समय | 60 s |

एक निष्क्रिय कनेक्शन की लागत लगभग 16 KB है, जिसे सर्वर और क्लाइंट को एक ही प्रोसेस में चलाकर मापा गया।
