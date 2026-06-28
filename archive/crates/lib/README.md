# Archived Library Crates

These crates are preserved for reference but are no longer part of the active MartyPC workspace.

- `frontend/display_manager_wgpu`: Old raw Winit/WGPU display manager used before the migration to the eframe frontend
- `backend/display_backend_wgpu`: WGPU display backend used by the old raw WGPU display manager.
- `frontend/marty_egui_wgpu`: Raw WGPU egui integration layer used by the old non-eframe WGPU frontend.
- `backend/wgpu_wrapper`: Local Pixels-like WGPU wrapper used by the old raw WGPU backend and egui integration.
- `backend/display_backend_pixels`: Older Pixels-based display backend. It depends on the removed workspace `pixels` dependency.
- `backend/display_backend_eframe`: Generic eframe display backend for a non-wgpu/non-glow path. Current eframe builds use either `display_backend_eframe_wgpu` or `display_backend_eframe_glow`.
