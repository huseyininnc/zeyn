//! Embeds the translated pages that exist: every `pages/<lang>/<file>.md` becomes an entry of
//! `TRANSLATIONS` in `$OUT_DIR/translations.rs`. A page missing in a language is simply not
//! listed, so the site serves the English page there and a test reports the gap.

use std::fmt::Write;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("pages");
    println!("cargo:rerun-if-changed=pages");
    let mut langs: Vec<_> = std::fs::read_dir(&root)
        .expect("pages folder")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    langs.sort_by_key(|e| e.file_name());
    let mut out = String::from("/// Translated page sources: language, file name, Markdown.\n");
    out.push_str("pub static TRANSLATIONS: &[(&str, &str, &str)] = &[\n");
    for lang in langs {
        let mut files: Vec<_> = std::fs::read_dir(lang.path())
            .expect("language folder")
            .filter_map(Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "md")
            })
            .collect();
        files.sort_by_key(|e| e.file_name());
        for file in files {
            let _ = writeln!(
                out,
                "    ({:?}, {:?}, include_str!({:?})),",
                lang.file_name().to_string_lossy(),
                file.file_name().to_string_lossy(),
                file.path().to_string_lossy()
            );
        }
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("translations.rs");
    std::fs::write(dest, out).expect("write translations.rs");
}
