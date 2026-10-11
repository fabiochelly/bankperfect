//! Icône et informations de version de l'exécutable Windows.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/bankperfect.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/icon/bankperfect.ico")
            .set("ProductName", "BankPerfect")
            .set("FileDescription", "BankPerfect")
            .set("LegalCopyright", "© Fabio Chelly — licence MIT");
        if let Err(e) = resource.compile() {
            println!("cargo:warning=icône Windows non intégrée : {e}");
        }
    }
}
