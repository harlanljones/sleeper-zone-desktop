//! Windows exe metadata: version info and file description shown in
//! Explorer properties and the Inno Setup installer pages.
//!
//! The `#[cfg(windows)]` gate is evaluated for the *host* (build
//! scripts compile for the host): active on Windows CI/host builds,
//! compiled out everywhere else, so Linux builds need no resource
//! compiler.

fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set(
            "FileDescription",
            "Sleeper Zone Desktop — live NFL fantasy tracker",
        );
        res.set("ProductName", "Sleeper Zone Desktop");
        res.set("CompanyName", "Sleeper Zone");
        res.set(
            "LegalCopyright",
            "Licensed MIT. Uses the Sleeper API and ESPN feeds.",
        );
        if let Err(e) = res.compile() {
            eprintln!("winres: resource compile failed: {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    {
        println!("cargo:rerun-if-changed=build.rs");
    }
}
