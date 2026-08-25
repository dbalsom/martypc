
# Library Crates

| Crate                         | Description                                                                                                                 |
|-------------------------------|-----------------------------------------------------------------------------------------------------------------------------|
| `display_backend_eframe_glow` | Implements the eframe Glow display backend and its OpenGL-backed display-target surfaces.                                   |
| `display_backend_eframe_wgpu` | Implements the eframe wgpu display backend and its GPU-backed display-target surfaces.                                      |
| `display_backend_trait`       | Defines shared display-backend abstractions.                                                                                |
| `display_manager_eframe`      | Coordinates eframe viewports, display targets, video renderers, scalers, screenshots, and egui contexts.                    |
| `marty_common`                | Provides types and utilities shared by the emulation core and frontends.                                                    |
| `marty_config`                | Handles reading configuration from the main config TOML, command-line arguments and query-strings on web                    |
| `marty_core`                  | The main emulator core crate. CPU, system and device emulation lives here.                                                  |
| `marty_display_common`        | Provides the display-manager and scaler subsystems.                                                                         |
| `marty_egui`                  | The main GUI crate. Implements egui menus, panels, dialogs, widgets, debugger windows, themes, and GUI state.               |
| `marty_egui_common`           | Defines minimal egui event types intended to be shared by UI crates.                                                        |
| `marty_egui_eframe`           | Provides the GUI rendering context that applies configured themes and presents `marty_egui` through the eframe frontend.    |
| `marty_frontend_common`       | Provides reusable frontend services for resources, machines, media, input, timing, deployment detection, and device sounds. |
| `marty_scaler_glow`           | Implements display scaling and CRT shader effects using Glow.                                                               |
| `marty_scaler_null`           | Implements a no-op scaler for display backends that do not support shader-based scaling.                                    |
| `marty_scaler_wgpu`           | Implements display scaling and CRT shader effects using wgpu.                                                               |
| `marty_vhd`                   | Provides support for creating, reading and working with VHD files.                                                          |
| `marty_videocard_renderer`    | Converts emulated video-card output into RGBA frames with aperture, aspect-ratio, and composite-video processing.           |
| `marty_web_helpers`           | Provides WASM helpers for fetching browser resources and writing log output to the web console.                             |
