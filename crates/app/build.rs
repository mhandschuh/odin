// libghostty for the terminal pane (ADR 0004): `./x` builds it on the Mac and says where (GHOSTTY_LIB_DIR). Without it
// (a type check in the sandbox) nothing is linked.
fn main() {
    println!("cargo:rerun-if-env-changed=GHOSTTY_LIB_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let Ok(dir) = std::env::var("GHOSTTY_LIB_DIR") else { return };
    println!("cargo:rustc-link-search=native={dir}");
    for f in [
        "Carbon",
        "AppKit",
        "Cocoa",
        "Metal",
        "QuartzCore",
        "CoreText",
        "CoreGraphics",
        "CoreFoundation",
        "Foundation",
        "IOSurface",
        "IOKit",
        "CoreVideo",
        "UniformTypeIdentifiers",
        "Security",
    ] {
        println!("cargo:rustc-link-lib=framework={f}");
    }
    println!("cargo:rustc-link-lib=c++");
}
