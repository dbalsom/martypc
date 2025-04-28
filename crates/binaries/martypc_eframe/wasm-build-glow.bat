set MARTYPC_URL_BASE=http://localhost:8080
set CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk build --release=true --no-default-features --features=wasm_glow --config Trunk.toml
