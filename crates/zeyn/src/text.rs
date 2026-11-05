//! Text in the form search compares. [`fold`] is what free text search ([`crate::DataQuery::q`])
//! uses on both sides, in memory and in SQL; use it for search key columns too, so a key written
//! by the app matches what users type.

/// The search form of `text`: lower case, the Turkish dotted and dotless `i` (`İ`, `ı`) as `i`,
/// accented Latin letters without their accents (`é` as `e`, `ñ` as `n`) and `ß`, `æ`, `œ`, `þ`
/// written out (`ss`, `ae`, `oe`, `th`). Everything else, punctuation and other scripts included,
/// stays as it is (in lower case), so the result has no length limit and no separators of its
/// own; [`crate::Slug::from_title`] is the form for addresses.
///
/// ```
/// use zeyn::text::fold;
/// assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
/// assert_eq!(fold("Año Pequeño"), "ano pequeno");
/// assert_eq!(fold("Straße MÜLLER"), "strasse muller");
/// ```
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c.to_ascii_lowercase());
            continue;
        }
        if c == 'İ' {
            out.push('i');
            continue;
        }
        for c in c.to_lowercase() {
            match letter(c) {
                Some(plain) => out.push_str(plain),
                None if c == '\u{307}' => {}
                None => out.push(c),
            }
        }
    }
    out
}

/// The plain ASCII form of an accented Latin letter (lower case), or `None` when it has none.
pub(crate) fn letter(c: char) -> Option<&'static str> {
    Some(match c {
        'ı' => "i",
        'ş' | 'ș' | 'ś' | 'š' => "s",
        'ğ' => "g",
        'ç' | 'ć' | 'č' => "c",
        'ö' | 'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' | 'ő' => "o",
        'ü' | 'ù' | 'ú' | 'û' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => "i",
        'ñ' | 'ń' | 'ň' => "n",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ď' | 'đ' | 'ð' => "d",
        'ť' | 'ţ' | 'ț' => "t",
        'ł' => "l",
        'ř' => "r",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        'þ' => "th",
        _ => return None,
    })
}

/// Letters SQL `TRANSLATE` maps one to one before `LOWER`, in upper and lower case, so a
/// database folds text like [`fold`] does. [`SQL_TO`] holds their plain forms in the same order.
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub(crate) const SQL_FROM: &str = "İıIşŞșȘśŚšŠğĞçÇćĆčČöÖòÒóÓôÔõÕøØōŌőŐüÜùÙúÚûÛūŪůŮűŰųŲàÀáÁâÂãÃäÄåÅāĀăĂąĄèÈéÉêÊëËēĒėĖęĘěĚìÌíÍîÎïÏīĪįĮñÑńŃňŇýÝÿŸźŹżŻžŽďĎđĐðÐťŤţŢțȚłŁřŘ";

/// Plain forms of [`SQL_FROM`].
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub(crate) const SQL_TO: &str = "iiissssssssggccccccoooooooooooooooouuuuuuuuuuuuuuuuaaaaaaaaaaaaaaaaaaeeeeeeeeeeeeeeeeiiiiiiiiiiiinnnnnnyyyyzzzzzzddddddttttttllrr";

/// Letters [`fold`] writes out as two, which SQL replaces after `LOWER`.
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub(crate) const SQL_PAIRS: [(&str, &str); 4] =
    [("ß", "ss"), ("æ", "ae"), ("œ", "oe"), ("þ", "th")];

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn turkish_i_folds_to_plain_i_in_both_cases() {
        assert_eq!(
            fold("İzmir IZMIR ızgara ISPARTA"),
            "izmir izmir izgara isparta"
        );
        assert_eq!(fold("ÇĞÖŞÜ çğöşü"), "cgosu cgosu");
    }

    #[test]
    fn spanish_and_german_accents_fold_and_sharp_s_is_written_out() {
        assert_eq!(fold("Niño ÁRBOL Corazón"), "nino arbol corazon");
        assert_eq!(fold("Größe Äpfel Straße"), "grosse apfel strasse");
    }

    #[test]
    fn other_scripts_and_punctuation_stay() {
        assert_eq!(fold("Москва, 東京!"), "москва, 東京!");
        assert_eq!(fold("a-b_c"), "a-b_c");
    }

    #[cfg(any(feature = "postgres", feature = "oracle"))]
    #[test]
    fn the_sql_tables_fold_like_fold() {
        let to: Vec<char> = super::SQL_TO.chars().collect();
        for (i, c) in super::SQL_FROM.chars().enumerate() {
            assert_eq!(fold(&c.to_string()), to[i].to_string(), "{c}");
        }
        for (from, to) in super::SQL_PAIRS {
            assert_eq!(fold(from), to);
        }
    }
}
