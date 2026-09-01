set MARTYPC_BASE_URL=http://localhost:8000
set CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk serve --release=true --no-default-features  --features=wasm_glow --config Trunk.toml
