// Embeds the app icon into the Windows .exe (Explorer / taskbar). Never fails the build.
fn main() {
    println!("cargo:rerun-if-changed=assets/DashDesigner.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/DashDesigner.ico");
        res.set("ProductName", "Backwoods Dash Designer");
        res.set("FileDescription", "Backwoods Dash Designer");
        res.set("CompanyName", "Backwoods Fabrications");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Windows icon not embedded: {e}");
        }
    }
}
