//! Gives the Windows program the CadForge icon (assets/logo/cadforge.ico, made by `logo::tests`).
fn main() {
    println!("cargo:rerun-if-changed=assets/logo/cadforge.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo/cadforge.ico");
        res.set("ProductName", "CadForge");
        res.set("FileDescription", "CadForge");
        if let Err(e) = res.compile() {
            println!("cargo:warning=the program icon was not embedded: {e}");
        }
    }
}
