// Stamps the build time into the binary, so `lethe --version` tells two
// builds of the same version apart, and rebuilds when the page or skills change.
fn main() {
    let out = std::process::Command::new("date").args(["-u", "+%Y-%m-%d %H:%M UTC"]).output();
    let built = out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    println!("cargo:rustc-env=LETHE_BUILT={}", built.trim());
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=skills");
    println!("cargo:rerun-if-changed=web/dist");
}
