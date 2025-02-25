use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if let Ok(target) = env::var("TARGET") {
        if target == "wasm32-unknown-emscripten" {
            // Run the emsdk_env.bat script
            let status = Command::new("cmd")
                .args(["/C", "path\\to\\emsdk_env.bat"])
                .status()
                .expect("Failed to run emsdk_env.bat");

            if !status.success() {
                panic!("emsdk_env.bat script failed to execute successfully");
            }
        }
    }
}
