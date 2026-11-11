//! Windows only: embed the app icon and version info (VERSIONINFO) into `f-builder.exe`.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `F_BUILDER_REQUIRE_WINRES=1` (set by
//! the release workflow) turns it into an error.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/f-builder.ico");
    println!("cargo:rerun-if-env-changed=F_BUILDER_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/f-builder.ico")
        .set("ProductName", "F-Builder")
        .set("FileDescription", "F-Builder: find out how any software works")
        .set("LegalCopyright", "Copyright (c) 2026 F-Eco contributors. Based on F-Builder by the ArtCraft team. MIT OR Apache-2.0.")
        .set("OriginalFilename", "f-builder.exe")
        .set("InternalName", "f-builder");
    if let Err(e) = res.compile() {
        if std::env::var_os("F_BUILDER_REQUIRE_WINRES").is_some() {
            println!("cargo::error=embedding Windows resources failed: {e}");
            return;
        }
        println!("cargo:warning=f-builder.exe built without icon/version resources: {e}");
    }
}
