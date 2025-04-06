set MARTYPC_URL_BASE=http://localhost:8080
set CARGO_UNSTABLE_BUILD_STD=std,panic_abort
trunk serve --release=false --no-default-features --features=all_video_cards,sound,use_wgpu --config Trunk.toml
