//! Reminds developers that the embedded dashboard comes from `web/dist/` (built by npm),
//! not from `cargo` automatically.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let index = std::path::Path::new(&manifest_dir).join("web/dist/index.html");
    println!("cargo:rerun-if-changed=web/dist/index.html");
    if !index.exists() {
        println!("cargo:warning=web/dist/index.html is missing — the embedded dashboard will be incomplete until you run: cd web && npm ci && npm run build");
    }
}
