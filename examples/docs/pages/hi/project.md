# स्रोत, लाइसेंस और सुरक्षा

Zeyn ओपन सोर्स है। यह पेज बताता है कि कोड कहाँ है, किस लाइसेंस के तहत है, उसकी टेस्टिंग कैसे होती है और सुरक्षा समस्या की रिपोर्ट कैसे करें।

## स्रोत कोड

कोड और issues GitHub पर हैं: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn)। crates [crates.io](https://crates.io/crates/zeyn) पर प्रकाशित होते हैं और उनका API संदर्भ [docs.rs](https://docs.rs/zeyn) पर है।

## लाइसेंस

Zeyn दो लाइसेंस के तहत है: [MIT लाइसेंस](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) या [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE), आप जो चुनें (`Cargo.toml` में `MIT OR Apache-2.0`)। योगदान भी इन्हीं दो लाइसेंस के तहत लिए जाते हैं।

## गुणवत्ता

रिपॉज़िटरी में 767 टेस्ट फ़ंक्शन हैं (crates और उदाहरणों में `#[test]` और `#[tokio::test]`, 2026-11-06 को गिने गए)। इनके अलावा doc comments के उदाहरण doctest के रूप में चलते हैं।

हर pull request और `dev` व `main` पर हर push पर CI चलता है:

- फ़ॉर्मैटिंग के लिए `cargo fmt --all --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`, यानी कोई भी lint चेतावनी बिल्ड को रोक देती है;
- `cargo test --workspace`, हर crate और उदाहरण के टेस्ट;
- `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email` और `accounts` फ़ीचर के साथ `cargo test -p zeyn`;
- `cargo test -p zeyn-cli`, जो फ़्रेमवर्क को उसके सर्वर फ़ीचर के बिना भी बिल्ड करता है;
- लाइसेंस, सुरक्षा सूचनाओं और प्रतिबंधित dependencies के लिए `cargo deny`।

हर रन के नतीजे [Actions पेज](https://github.com/huseyininnc/zeyn/actions) पर सार्वजनिक हैं।

## सुरक्षा

- **समर्थित संस्करण।** Zeyn alpha में है। सुरक्षा सुधार सिर्फ़ नवीनतम `0.0.x` रिलीज़ को मिलते हैं।
- **रिपोर्ट करना।** किसी कमज़ोरी की रिपोर्ट GitHub Security Advisories से निजी तौर पर करें: रिपॉज़िटरी का Security टैब खोलें और "Report a vulnerability" चुनें ([सीधा लिंक](https://github.com/huseyininnc/zeyn/security/advisories/new))। इसकी रिपोर्ट कभी किसी सार्वजनिक issue, चर्चा या pull request में न करें।
- **जवाब।** रिपोर्ट मिलने की पुष्टि 5 कार्य दिवसों के भीतर की जाती है। सुधार रिलीज़ होने के बाद एक सुरक्षा सूचना प्रकाशित होती है और रिपोर्ट करने वाले का नाम दिया जाता है, जब तक वे नाम न देने को न कहें।

रिपोर्ट में क्या शामिल करें और दायरा क्या है, इसके साथ पूरी नीति [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md) में है।

## और

- [बदलाव सूची](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): हर रिलीज़ के बदलाव।
- [रोडमैप](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): अगली रिलीज़ और योजनाएँ।
- [योगदान गाइड](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): बिल्ड, टेस्ट और बदलाव भेजने का तरीका।
- [आचार संहिता](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): हम साथ कैसे काम करते हैं।
- [सहायता](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): सवाल पूछने और बग रिपोर्ट करने की जगह।
