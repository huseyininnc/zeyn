//! For the `desktop` feature on Windows: embeds the icon, the version info and the manifest
//! into the exe. Other builds do nothing here.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/adisyon.ico")
            .set("ProductName", "Adisyon")
            .set("FileDescription", "Adisyon")
            .set("OriginalFilename", "zeyn-example-adisyon.exe")
            .set_manifest_file("assets/adisyon.exe.manifest");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon and version info were not embedded: {e}");
        }
    }
}
