set MARTYPC_URL_BASE=http://localhost:8080
set CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk serve --no-default-features --features=wasm_wgpu --config Trunk.toml
