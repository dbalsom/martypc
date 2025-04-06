
# MartyPC Frontend Crates

Here are various frontends for MartyPC.

### martypc_desktop_wgpu
 - This is a frontend that uses raw winit and wgpu, rendering egui as a separate layer to allow for 
   custom shaders.  It is currently broken due to winit 0.30 completely changing its API and wgpu 
   adding lifetimes that broke my DisplayManager trait. I hate lifetime annotations.

### martypc_eframe
 - This is a frontend build on top of [eframe](https://github.com/emilk/egui/tree/master/crates/eframe). 
   It is up to date with winit and wgpu dependencies, but currently lacks some of the features of the
   old marty_desktop_wgpu frontend, namely shaders. This is probably what you should build.

   This frontend can be built for the web using the `wasm-unknown-unknown` target.
   You can build and serve the web version locally using `trunk serve`.

### martypc_headless
 - This is a headless, cli-only frontend for MartyPC. This is used to generate or validate CPU tests and
   perform benchmarks.

### martypc_web_player_wgpu
 - This was the old wasm build of MartyPC used to make the old web demos - it's hopelessly code-rotten now and will not build.
   I will probably remove it at some point.