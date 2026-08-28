use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/Cargo.toml");

    let status = Command::new("trunk")
        .args(["build", "--release"])
        .current_dir("web")
        .status()
        .expect("failed to run `trunk build` - install trunk with `cargo install trunk` and the wasm32-unknown-unknown target with `rustup target add wasm32-unknown-unknown`");

    if !status.success() {
        panic!("trunk build failed with status {status}");
    }
}
