use std::env::var;

use winresource::WindowsResource;

// Explorer and the taskbar read the app icon from the exe resource, the
// runtime winit icon covers only the open window.
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    if var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    WindowsResource::new()
        .set_icon("../../assets/icon.ico")
        .compile()
        .expect("embed the windows icon resource");
}
