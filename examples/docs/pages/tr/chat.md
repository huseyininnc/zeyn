# Sohbet

Konuşmalar için bileşenler: bir konuşmanın mesajları, kullanıcının yazdığı kutu ve birinin yazmakta olduğunu bildiren not. Her biri sunucuda çizilir ve stil dosyasını ve betiğini yalnızca onu kullanan sayfalarda yükler.

## MessageThread

`MessageThread` bir konuşmanın mesajlarını sohbet balonları olarak gösterir: kullanıcının kendi mesajları bitiş tarafında, diğerleri başlangıç tarafında (sağdan sola yazılan dillerde aynalanır), bir göndericinin beş dakika içindeki ardışık mesajları tek bir ad altında gruplanır, kullanıcının saat diliminde her gün için bir başlık, her mesajın saati ve kendi mesajlarında teslim durumu gösterilir.

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

Geçmiş action'ı bir `ThreadQuery` alır: sunucunun bileşenden aldığı konuşma (tarayıcı bunu hiç göndermez) ve en yeniden eskiye sıralı bir `DataQuery`. Action içinde kullanıcının konuşmanın bir katılımcısı olduğunu denetleyin. `.author` göndericinin kullanıcı kimliğidir; kendi mesajlarını diğerlerinden ayırmak için oturum açmış kullanıcıyla karşılaştırılır. `.name` gösterilen addır. Modelin bir `#[key]` alanı olmalıdır; böylece iki kez yüklenen bir mesaj bir kez gösterilir.

İlk 50 mesaj (`THREAD_PAGE`) ekranla birlikte çizilir. Yukarı kaydırmak bir sonraki eski sayfayı `POST /_zeyn/grid` üzerinden yükler; ekranın erişimi ve action'ın izni her seferinde denetlenir ve okuyucunun baktığı mesaj yerinde kalır. Sayfada en fazla 200 mesaj tutulur: daha ileri gitmek uzak ucu bırakır, okuyucu geri döndüğünde o kısım yeniden yüklenir. Böylece binlerce mesajlık bir konuşma kısa bir konuşmayla aynı belleği harcar. 5.000 mesaj arasında kaydırma, sayfada yaklaşık 2.000 öğeyle ekranın kare hızında çalışır.

`Delivery` durumları `Sending`, `Sent`, `Delivered`, `Read` (okundu bilgisi) ve `Failed` şeklindedir; her biri bir simge ve ekran okuyucular için bir metinle gösterilir. `.attachments(field, rules)` bir mesajın dosyalarını gösterir (bir `Vec<File>` alanı ve `Feature::files` ile kayıtlı bir deponun kuralları): görseller sabit bir kutuda, bir yer tutucunun üzerinde tembel olarak yüklenir; diğer dosyalar deponun okuma izninden geçen indirme bağlantıları olarak gösterilir.

Yeni mesajlar, thread'in topic'lerinden biri anıldığında getirilir; `.topic(..)` başka bir şey söylemedikçe bu `"chat"` olur: bir `Composer` gönderdikten sonra, bir action `ui.refresh("chat")` ile bittikten sonra ya da `.live()` ve `realtime` özelliğiyle herhangi bir yerde `hub.notify(&ctx, "chat")` çalıştığında. Başkalarının mesajları kibar (polite) bir canlı bölgede duyurulur.

### Yapay zekâ yanıtları

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

`.reply(action)` ile, aynı konuşmanın bir `Composer`'ıyla gönderilen mesaj bir yapay zekâ yanıtı başlatır: tarayıcı metni `POST /_zeyn/stream` adresine gönderir (CSRF, yazma sınırı, ekranın erişimi, action'ın izni ve interceptor'lar) ve yanıt, gömülü asistanın biçiminde JSON satırları olarak akarak geri gelir. Parçalar sınırlı bir kanaldan (`REPLY_BUFFER`) geçer; sunucu `MAX_REPLY_CHARS` (32.000) karakterden veya `REPLY_SECONDS` (120) saniyeden sonra durur ve bir durdurma düğmesi yanıtı erken bitirir. `ReplySender::send`, okuyucu gittiğinde `false` döner; bu, üreticinin durma işaretidir.

Yanıt yalnızca metin düğümlerinden kurulan küçük bir Markdown alt kümesi olarak gösterilir; böylece modelin yazdığı hiçbir şey HTML olarak ayrıştırılmaz. Önceki mesajlar asla yeniden çizilmez, yalnızca yazılmakta olan paragraf çizilir: 2.000 parçalık bir yanıt sonunda da başındaki kadar kare başına maliyet çıkarır ve üstündeki hiçbir şey kaymaz. Canlı bölge yanıtı her parçada değil, bittiğinde bir kez okur. `.ai(field)` ile işaretlenen mesajlar da geçmişten güvenli Markdown olarak yüklenir (`markdown` özelliğiyle; onsuz düz metin olarak).

Yanıtın anahtarı (`Reply::channel(key)`), saklanan mesajın sahip olacağı anahtardır; böylece geçmiş ikinci bir kopya yerine saklanan yanıtı kendi yerinde gösterir.

## Composer

`Composer`, kullanıcının mesaj yazdığı kutudur: metniyle birlikte en fazla bir yüksekliğe kadar büyüyen bir metin alanı, bir gönder düğmesi ve istenirse ekler, anmalar ve saklanan bir taslak.

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, hub: Hub, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    hub.notify(&ctx, "chat");
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

Gönderme action'ı bir `ChatSend` alır: sunucunun bileşenden aldığı konuşma, kırpılmış metin, eklenen dosyalar ve anılan kişilerin kimlikleri. Gönderme CSRF belirteciyle `POST /_zeyn/data` üzerinden yapılır, yazma sınırından düşer ve action'ın iznini gerektirir; kullanıcının gönderemeyeceği bir composer sayfaya konmaz. Sunucu boş bir mesajı, `.max_chars(n)` değerinden (ayarlanmadıysa 4.000) uzun bir mesajı, deponun kurallarının izin verdiğinden fazla dosyayı, biçimi yanlış dosya kimliklerini ve 20'den fazla anmayı reddeder. Geri kalanını da güvenilmez sayın: kullanıcının konuşmaya katıldığını ve anılan kişileri anabileceğini denetleyin.

Varsayılan olan `SendKey::Enter`, Enter ile gönderir ve Shift+Enter ile yeni satır açar; `.send_key(SendKey::CtrlEnter)` Ctrl+Enter ile (Mac'te Cmd+Enter) gönderir ve Enter'ı yeni satırlara bırakır. Mesaj boşken veya çok uzunken gönder düğmesi devre dışıdır ve sınıra yaklaşınca bir sayaç görünür. Gönderdikten sonra composer topic'lerini anar (`.topic(..)` başka bir şey söylemedikçe `"chat"`); böylece aynı konuşmanın bir `MessageThread`'i mesajı hemen gösterir ve `.reply(..)` varsa yapay zekâ yanıtını akıtır.

`.attachments(rules)`, `Feature::files` ile kayıtlı bir deponun yükleme alanını ekler: dosyalar bir ilerleme çubuğuyla tek tek `POST /_zeyn/files/{store}` adresine yüklenir, deponun yazma izni ve tür denetimlerinden geçer ve kimlikleri mesajla birlikte gider.

`.mentions(action)`, `@` işaretinden sonra bir lookup action'ıyla kişi arar (girdi `LookupQuery`, çıktı bir `LookupItem` sayfası). Arama son tuştan 200 ms sonra başlar, yeni bir arama eskisini iptal eder ve sunucu en fazla `MAX_MENTIONS` (8) kişi ister ve en fazla o kadarını geri gönderir. Liste, metin alanından yönetilen bir listbox'tır: oklar hareket ettirir, Enter veya Tab seçer, Escape kapatır.

`.remember(key)` her konuşmanın gönderilmemiş metnini, gönderilene kadar istemci tercih deposunda `App::client_prefs` ile kayıtlı bir `Draft` anahtarı altında tutar. Bir taslak en fazla `MAX_DRAFT_CHARS` (4.000) karakterdir, en fazla `MAX_DRAFTS` (20) konuşma tutulur ve önce en eskisi silinir. Konuşma kimliği yalnızca bir hash olarak saklanır. `Purpose::Preference` ile taslaklar, kullanıcı daha fazlasına onay verene kadar oturum boyunca kalır.

## TypingIndicator

`TypingIndicator`, bir konuşmada başka kimin yazdığını kibar (polite) bir canlı bölgede gösterir: "Ada yazıyor", "Ada ve Grace yazıyor", "3 kişi yazıyor".

```rust
let realtime = Realtime::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

Gerçek zamanlı kanal üzerinden (`realtime` özelliği, bkz. `realtime.md`), `Realtime::topic` ile bildirilen bir topic'te çalışır. Kullanıcı aynı konuşmanın bir `Composer`'ında yazarken tarayıcı, sayfanın canlı soketinden en fazla 3 saniyede bir küçük bir sinyal gönderir; sunucu kullanıcı başına daha sık gelenleri atar ve sinyali tenant içinde topic'in diğer abonelerine iletir, aynı kullanıcıya asla geri göndermez. Bir sinyal konuşma kimliğini ve kullanıcının yalnızca bu sunucunun üretebileceği bir anahtarını taşır; kullanıcı kimliğini veya herhangi bir metni asla taşımaz. Bir ad, son sinyalden sonra 6 saniye gösterilir; `.person(user, name)` ile verilmeyen kişiler "biri" olarak görünür.

`realtime` özelliği olmadan ya da uygulamanın gerçek zamanlı kanalı yoksa gösterge hiçbir şey çizmez, betiği hiç yüklenmez ve hiçbir şey gönderilmez. Sinyaller tenant içinde topic'in her abonesine ulaşır, dolayısıyla konuşma kimliği onlara görünür: opak kimlikler ya da her kitle için ayrı bir topic kullanın.
