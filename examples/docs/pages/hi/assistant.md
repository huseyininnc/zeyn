# एम्बेडेड असिस्टेंट

स्थिति: `zeyn` के `assistant` Cargo feature के पीछे लागू।

Zeyn ऐप अपना AI असिस्टेंट रख सकता है: हर पृष्ठ पर एक चैट जो ऐप के डेटा से उत्तर देती है और साइन इन उपयोगकर्ता के रूप में ऐप की Screen और API चलाती है। Zeyn से बने ऐप इसे अपने ग्राहकों के लिए चालू कर सकते हैं, वह मॉडल जोड़ सकते हैं जो उनके ग्राहक चाहें, और चैट इतिहास वहाँ रख सकते हैं जहाँ उन्हें ज़रूरत हो।

```toml
zeyn = { version = "0.0.10", features = ["assistant"] }
```

feature के बिना इसका कुछ भी कंपाइल नहीं होता: कोई HTTP क्लाइंट नहीं, कोई अतिरिक्त रूट नहीं, कोई अतिरिक्त मेमोरी नहीं।

## 1. सेटअप

```rust
use zeyn::assistant::{Assistant, OpenAiCompatible};

let model = OpenAiCompatible::new(config.require("assistant.base_url")?, config.require("assistant.model")?)
    .api_key(config.secret("assistant.api_key")?);

App::new("Lifts")
    .assistant(
        Assistant::new(model)
            .permission("lifts.assistant.use")
            .instructions("You help service engineers plan elevator maintenance.")
            .suggest(work_orders, "Which lifts are overdue for service?")
            .suggest_anywhere("What can you do?"),
    )
```

बस इतना ही। हर पृष्ठ को एक तैरता लॉन्चर मिलता है जो चैट को साइड पैनल के रूप में खोलता है। Screen चैट को खुद भी दिखा सकती है:

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` अन्य सामग्री के बगल में फ़िट होता है। `Assistant::launcher(false)` तैरता लॉन्चर हटाता है, इसलिए चैट केवल वहाँ दिखती है जहाँ Screen उसे रखे।

| विकल्प | डिफ़ॉल्ट |
|---|---|
| `permission("...")` | हर साइन इन उपयोगकर्ता |
| `instructions("...")` | कोई नहीं; ऐप के डेवलपर लिखते हैं, उपयोगकर्ता कभी नहीं |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | कोई शुरुआती चिप नहीं |
| `speech_to_text(p)`, `text_to_speech(p)` | ब्राउज़र का Web Speech API |
| `no_voice()` | माइक्रोफ़ोन और ज़ोर से पढ़ना चालू |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | असिस्टेंट पुष्टि के बाद सहेज भी सकता है |
| `rate_limit(RateLimit)` | प्रति उपयोगकर्ता प्रति मिनट 20 अनुरोध |
| `token_limit(Some(RateLimit))` | प्रति उपयोगकर्ता प्रति घंटा 200,000 टोकन |
| `limits(Limits { .. })` | खंड 6 देखें |

## 2. मॉडल

`LlmProvider` लागू करने वाला कोई भी टाइप काम करता है: एक मेथड जो स्ट्रीम किया राउंड शुरू करती है और `ChatEvent` (टेक्स्ट के टुकड़े, टूल कॉल, उपयोग) का सीमित चैनल लौटाती है। बिल्ट इन:

| प्रोवाइडर | क्या कवर करता है |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI (`https://api.openai.com/v1`), Azure OpenAI v1 API (`https://NAME.openai.azure.com/openai/v1`, मॉडल के रूप में deployment, `api-key` हेडर के लिए `.azure()`), Ollama (`http://localhost:11434/v1`), vLLM, LiteLLM और अन्य संगत सर्वर। उन सर्वर के लिए `.legacy_max_tokens()` जो `max_completion_tokens` नहीं जानते |
| `Anthropic::new(model)` | Anthropic Messages API (गेटवे के लिए `.base_url(...)`)। Reasoning ब्लॉक एक टर्न के भीतर बिना बदले वापस भेजे जाते हैं |
| `ScriptedProvider` | टेस्ट, डेमो और ऑफ़लाइन डेवलपमेंट: नेटवर्क कॉल की जगह नियम, शब्द दर शब्द स्ट्रीम, असली टूल कॉल |

कुंजियाँ `Secret` मानों (`Config::secret`) से आती हैं, सर्वर पर रहती हैं और त्रुटि संदेशों में कभी नहीं दिखतीं। प्रोवाइडर त्रुटियाँ सर्वर पर लॉग होती हैं; उपयोगकर्ता को सामान्य संदेश दिखता है।

शोकेस को असली मॉडल की ओर इंगित करने में केवल कॉन्फ़िगरेशन लगता है:

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

या `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5` के साथ `NORTHWIND_ASSISTANT__PROVIDER=anthropic`, या बिना कुंजी के Ollama URL। `NORTHWIND_ASSISTANT__PROVIDER=off` इसे बंद करता है।

## 3. स्पीच

डिफ़ॉल्ट रूप से माइक्रोफ़ोन ब्राउज़र की स्पीच रिकग्निशन और "ज़ोर से पढ़ें" ब्राउज़र की स्पीच सिंथेसिस इस्तेमाल करते हैं: कोई सर्वर लागत नहीं, ऐप के लिए कोई ऑडियो ब्राउज़र से बाहर नहीं जाता। सर्वर स्पीच के लिए:

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

दोनों OpenAI संगत `/audio/transcriptions` और `/audio/speech` एंडपॉइंट बोलते हैं, जो स्थानीय Whisper और स्पीच सर्वर भी देते हैं। रिकॉर्डिंग सीमित हैं (ब्राउज़र में 4 MB, 60 सेकंड); स्पीच ऑडियो आते ही ब्राउज़र तक स्ट्रीम होता है।

## 4. इतिहास

`ChatStore` बातचीत को टेनेंट और उपयोगकर्ता (`ChatKey`) की कुंजी से रखता है, इसलिए टेनेंट कभी एक साझा नहीं करते:

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` डिफ़ॉल्ट है (60 संदेशों की 1,000 बातचीत, सबसे कम हाल में इस्तेमाल हुई पहले बाहर, रीस्टार्ट पर खो जाती हैं)। डेटाबेस के लिए तीनों मेथड को टेनेंट, उपयोगकर्ता, स्थिति और JSON के रूप में संदेश वाली तालिका पर लागू करें। संदेश केवल जोड़े जाते हैं; स्टोर प्रतिधारण तय करता है। लंबित पुष्टियाँ भी इतिहास में रहती हैं, इसलिए सर्वर कोई अन्य प्रति उपयोगकर्ता स्थिति नहीं रखता।

## 5. टूल और action

मॉडल वही टूल देखता है जो MCP और WebMCP (देखें `docs/runtime-protocol.md` खंड 9) में हैं, हर बार साइन इन उपयोगकर्ता के लिए बने: ग्रिड खोज, फ़ॉर्म सेव और API action जिन्हें उपयोगकर्ता इस्तेमाल कर सकता है, `ai = false` चिह्नित हर चीज़ को छोड़कर, `///` डॉक टिप्पणियों और `ai = "..."` मार्गदर्शन के विवरण के साथ। दो बिल्ट इन टूल जोड़े जाते हैं:

- `navigate`: उपयोगकर्ता जो रूट वाली Screen खोल सकता है उसे बिना पृष्ठ लोड के खोलता है (`zeyn:navigate`), और उसके ग्रिड को फ़िल्टर कर सकता है (`zeyn:filter`)। ग्रिड फिर `POST /_zeyn/grid` से लोड होता है, जहाँ सर्वर अपनी सामान्य जाँचें लागू करता है।
- `suggest`: अनुवर्ती प्रॉम्प्ट जो चिप के रूप में दिखते हैं।

हर कॉल सामान्य action पथ से गुज़रती है, कॉन्टेक्स्ट को एजेंट के रूप में चिह्नित करके: Screen का एक्सेस, feature के गार्ड, action की अनुमति, इंटरसेप्टर, दर सीमाएँ, समय सीमा, ऑडिट (हर असिस्टेंट कॉल `via_agent: true` के साथ दर्ज होती है) और मास्किंग।

**पुष्टि।** पठन तुरंत चलते हैं। जो कुछ भी लिखता है (फ़ॉर्म सेव, API action जिनका इनपुट `DataQuery` नहीं है) और `#[action(confirm)]` चिह्नित हर action राउंड को रोकती है और पुष्टि कार्ड दिखाती है जिसमें दिखता है क्या चलेगा और उसके आर्ग्युमेंट। केवल उपयोगकर्ता का क्लिक (`POST /_zeyn/assistant/confirm`, CSRF के साथ, उस उपयोगकर्ता के अपने इतिहास में लंबित कॉल के लिए) उसे एक बार चलाता है। "I confirm" जैसा संदेश केवल टेक्स्ट है: नया संदेश लंबित कॉल को न चली के रूप में बंद कर देता है।

## 6. सुरक्षा मॉडल

| जोखिम | फ्रेमवर्क क्या करता है |
|---|---|
| लीक हुई API कुंजियाँ | कुंजियाँ सर्वर पर `Secret` हैं; ब्राउज़र केवल ऐप से बात करता है |
| मॉडल का उपयोगकर्ता से आगे काम करना | टूल उपयोगकर्ता की अनुमतियों से बनते हैं; हर कॉल सर्वर पर फिर जाँची जाती है |
| वे लेखन जो उपयोगकर्ता नहीं चाहता था | लेखन और `confirm` action को उपयोगकर्ता का क्लिक चाहिए; मॉडल पुष्टि नहीं कर सकता |
| मास्क किया डेटा मॉडल तक पहुँचना | मास्क किए फ़ील्ड एजेंट कॉल के लिए हमेशा मास्क रहते हैं, उन उपयोगकर्ताओं के लिए भी जो reveal कर सकते हैं |
| डेटा के ज़रिए प्रॉम्प्ट इंजेक्शन | टूल परिणाम `<tool_result>` सीमांकक के भीतर सादे टेक्स्ट हैं जिनमें `<` और `>` escape किए गए हैं; सिस्टम प्रॉम्प्ट कहता है कि वे डेटा हैं। सिस्टम प्रॉम्प्ट सर्वर हर राउंड निश्चित नियमों, ऐप के निर्देशों और Screen के नाम से दोबारा बनाता है, कभी उपयोगकर्ता, पृष्ठ या टूल डेटा से नहीं |
| लिंक के ज़रिए डेटा चोरी | उत्तर `textContent` से बने छोटे Markdown उपसमुच्चय के रूप में रेंडर होते हैं: कोई HTML नहीं, कोई लिंक नहीं, कोई छवि नहीं |
| बेकाबू लागत | प्रति उपयोगकर्ता अनुरोध दर सीमा, प्रति उपयोगकर्ता टोकन सीमा, सीमित आउटपुट टोकन, अधिकतम 6 राउंड और प्रति राउंड 4 टूल कॉल |
| बड़े इनपुट | 2,000 अक्षरों तक संदेश, प्रति राउंड 30 इतिहास संदेश, टूल परिणाम 6,000 अक्षरों तक कटे, रिकॉर्डिंग 4 MB तक |
| क्रॉस साइट अनुरोध | हर `POST` को CSRF हेडर चाहिए और वह ऐप की लेखन सीमा में गिना जाता है |
| जवाबदेही रहित कार्रवाइयाँ | हर असिस्टेंट टूल कॉल `via_agent` के साथ ऑडिट होती है |

यह जो नहीं करता: तय करना कि मॉडल का उत्तर सही है या नहीं। उपयोगकर्ताओं को यह बताएँ (पैनल बताता है) और जो action मायने रखती हैं उन्हें `confirm` के पीछे रखें।

## 7. ग्राहकों के लिए एम्बेड करना

जो ऐप असिस्टेंट अपने ग्राहकों को बेचता है उसे आमतौर पर चाहिए:

- `permission("app.assistant.use")` और गार्ड में लाइसेंस मॉड्यूल जाँच, ताकि केवल भुगतान करने वाले टेनेंट इसे देखें;
- प्रति टेनेंट मॉडल: `LlmProvider` को छोटे राउटर के रूप में लागू करें जो `ctx` टेनेंट से कॉन्फ़िगरेशन में से ग्राहक का प्रोवाइडर चुने, या सबके लिए एक मॉडल चलाएँ;
- ऐप के डेटाबेस पर `ChatStore`, ग्राहक के प्रतिधारण नियमों के साथ;
- `instructions(...)` जो डोमेन का वर्णन करें (लिफ़्ट कंपनी: इकाइयाँ, साइटें, सर्विस अंतराल), और जिन action को चाहिए उन पर `ai = "..."` मार्गदर्शन;
- `token_limit` उस प्लान पर सेट जिसका ग्राहक भुगतान करता है।

## 8. एंडपॉइंट

| एंडपॉइंट | क्या करता है |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | `application/x-ndjson` घटनाएँ स्ट्रीम करता है |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | लंबित कॉल चलाता या अस्वीकार करता है, फिर चैट की तरह स्ट्रीम करता है |
| `POST /_zeyn/assistant/clear` | बातचीत हटाता है; `204` |
| `POST /_zeyn/assistant/stt` (ऑडियो बॉडी) | `{ text }`; सर्वर speech to text के बिना `404` |
| `POST /_zeyn/assistant/tts` `{ text }` | स्ट्रीम किया ऑडियो; सर्वर text to speech के बिना `404` |

घटनाएँ, प्रति पंक्ति एक JSON ऑब्जेक्ट: `{t:"text",v}`, `{t:"tool",name,id}`, `{t:"navigate",url,screen,filters}`, `{t:"suggest",v}`, `{t:"confirm",id,tool,title,question,details}`, `{t:"resolved",id,approved}`, `{t:"error",message}`, `{t:"done"}`।

`assistant.js` और `assistant.css` केवल उन उपयोगकर्ताओं के लिए लोड होते हैं जो असिस्टेंट इस्तेमाल कर सकते हैं। चैट लॉग एक `role="log"` लाइव रीजन है (उत्तर स्ट्रीम होते समय `aria-busy`), Enter भेजता है, Shift+Enter नई पंक्ति शुरू करता है, Escape साइड पैनल बंद करता है और फ़ोकस लॉन्चर पर लौटाता है, और पुष्टि कार्ड पहले Cancel पर फ़ोकस रखता है।

## 9. लागत

असिस्टेंट मेमोरी में सीमित इतिहास स्टोर और चलती स्ट्रीम को छोड़कर कोई प्रति उपयोगकर्ता स्थिति नहीं रखता। उत्तर 32 आइटम के चैनलों से अंत से अंत तक स्ट्रीम होते हैं। शोकेस पर मापा गया (डिबग बिल्ड, Windows, पाँच पृष्ठ लोड के बाद निष्क्रिय): असिस्टेंट बंद होने पर 14.4 MB working set और 3.13 MB निजी मेमोरी, चालू होने पर 14.5 MB और 3.19 MB (लगभग 100 KB अधिक)।
