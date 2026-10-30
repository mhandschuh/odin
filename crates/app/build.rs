// libghostty for the terminal pane (ADR 0004): `./x` builds it on the Mac and passes the library's path
// (GHOSTTY_LIB; in Ghostty 1.4 libghostty-internal.a). Without it (a type check in the sandbox) nothing is linked.
fn main() {
    println!("cargo:rerun-if-env-changed=GHOSTTY_LIB");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let Ok(lib) = std::env::var("GHOSTTY_LIB") else { return };
    let lib = std::path::Path::new(&lib);
    let dir = lib.parent().map(|d| d.display().to_string()).unwrap_or_default();
    let name = lib.file_stem().and_then(|s| s.to_str()).unwrap_or("ghostty").trim_start_matches("lib").to_string();
    println!("cargo:rerun-if-changed={}", lib.display());
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static={name}");
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
