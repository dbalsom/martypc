/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------
*/

use marty_egui::{state::GuiState, themes::make_theme};
use marty_frontend_common::{GuiContextOptions, MartyGuiTheme};

use egui::{Color32, Context};
use egui_extras::install_image_loaders;

//use web_time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct GuiFrameOutput {
    pub capture_state:   Option<bool>,
    pub main_panel_rect: egui::Rect,
}

/// Manages all state required for rendering egui over `Pixels`.
#[allow(dead_code)]
pub struct GuiRenderContext {
    /// Cloned egui context, in case we need to access it.
    ctx: Context,
    /// The unthemed style inherited from the egui context.
    base_style: egui::Style,
    /// The currently selected main theme.
    main_theme: MartyGuiTheme,
    /// The style to use for the main UI.
    main_style: egui::Style,
    /// The style to use for the menu UI.
    menu_style: egui::Style,
    /// The global scale factor for the UI.
    scale_factor: f64,
}

impl Default for GuiRenderContext {
    fn default() -> Self {
        let ctx = Context::default();
        let base_style = (*ctx.global_style()).clone();
        let main_theme = MartyGuiTheme::default();
        let main_style = Self::resolve_theme(&base_style, main_theme);
        let menu_style = main_style.clone();
        ctx.set_global_style(main_style.clone());
        Self {
            ctx,
            base_style,
            main_theme,
            main_style,
            menu_style,
            scale_factor: 1.0,
        }
    }
}

impl GuiRenderContext {
    /// Create egui.
    pub fn new(
        ctx: egui::Context,
        dt_idx: usize,
        width: u32,
        height: u32,
        scale_factor: f64,
        gui_options: &GuiContextOptions,
    ) -> Self {
        //let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
        //let egui_ctx = Context::default();

        log::debug!(
            "GuiRenderContext::new(): {}x{} (scale_factor: {} native_scale_factor: {})",
            width,
            height,
            scale_factor,
            ctx.native_pixels_per_point().unwrap_or(1.0)
        );

        // Required to initialize image loaders from egui_extras. Features control what loaders
        // will be installed.
        install_image_loaders(&ctx);

        let _id_string = format!("display{}", dt_idx);

        ctx.set_zoom_factor(gui_options.zoom.min(1.0).max(0.1));

        //egui_state.set_max_texture_side(max_texture_size);

        // let screen_descriptor = ScreenDescriptor {
        //     size_in_pixels:   [width, height],
        //     pixels_per_point: scale_factor as f32,
        // };

        //let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        //let textures = TexturesDelta::default();

        // Resolve themes.
        let gui_theme_enum = gui_options.theme.unwrap_or_default();
        let menu_theme_enum = gui_options.menu_theme.unwrap_or(gui_theme_enum);
        let base_style = (*ctx.global_style()).clone();
        let main_style = Self::resolve_theme(&base_style, gui_theme_enum);
        let menu_style = Self::resolve_theme(&base_style, menu_theme_enum);
        ctx.set_global_style(main_style.clone());

        #[cfg(debug_assertions)]
        if gui_options.debug_drawing {
            ctx.set_debug_on_hover(true);
        }

        let slf = Self {
            ctx,
            base_style,
            main_theme: gui_theme_enum,
            main_style,
            menu_style,
            scale_factor,
        };

        //slf.resize(width, height);
        slf
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn ctx_mut(&mut self) -> &mut Context {
        &mut self.ctx
    }

    fn resolve_theme(base_style: &egui::Style, theme: MartyGuiTheme) -> egui::Style {
        use egui::{FontFamily::Proportional, FontId, TextStyle::*};

        let theme = make_theme(theme);
        let mut style = base_style.clone();
        theme.apply_to_style(&mut style);

        // Make headers smaller, regardless of theme.
        style.text_styles.entry(Heading).and_modify(|text_style| {
            *text_style = FontId::new(14.0, Proportional);
        });

        style
    }

    fn set_main_theme(&mut self, theme: MartyGuiTheme) {
        self.main_theme = theme;
        self.main_style = Self::resolve_theme(&self.base_style, theme);
        self.ctx.set_global_style(self.main_style.clone());
    }

    pub fn show<Fw, Fm>(
        &mut self,
        root_ui: &mut egui::Ui,
        state: &mut GuiState,
        show_menu: bool,
        show_windows: bool,
        main_panel_fill: Option<Color32>,
        mut window_render: Fw,
        mut main_panel_render: Fm,
    ) -> GuiFrameOutput
    where
        Fw: FnMut(&mut egui::Context, &mut GuiState, &mut Option<bool>),
        Fm: FnMut(&mut egui::Ui, &mut GuiState, egui::Rect, &mut Option<bool>),
    {
        let mut capture_state = None;

        if show_menu {
            self.ctx.set_global_style(self.menu_style.clone());
            *root_ui.style_mut() = self.menu_style.clone();
            egui::Panel::top("martypc_top_panel").show_inside(root_ui, |ui| state.show_menu(ui));
        }

        let current_theme = state.current_theme();
        if current_theme != self.main_theme {
            self.set_main_theme(current_theme);
        }

        self.ctx.set_global_style(self.main_style.clone());
        *root_ui.style_mut() = self.main_style.clone();
        let main_visuals = self.main_style.visuals.clone();

        state.update_osd_keyboard_unhide_gesture(&self.ctx, root_ui.available_rect_before_wrap());
        if state.osd_keyboard_enabled() {
            state.show_osd_keyboard_panel(root_ui);
        }

        if show_windows {
            state.show_windows(&self.ctx);
        }

        let old_margin = self.ctx.global_style().spacing.window_margin;
        // Disable window margin for display window.
        self.ctx.global_style_mut(|style| {
            style.spacing.window_margin = egui::Margin::ZERO;
        });
        window_render(&mut self.ctx, state, &mut capture_state);
        // Restore window margin.
        self.ctx.global_style_mut(|style| {
            style.spacing.window_margin = old_margin;
        });

        // Override panel fill if requested.
        let mut panel_frame = egui::Frame::default();
        panel_frame.inner_margin = egui::Margin::ZERO;
        panel_frame.fill = main_visuals.panel_fill;
        if let Some(fill) = main_panel_fill {
            panel_frame.fill = fill;
        }
        let main_panel_rect = egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(root_ui, |ui| {
                ui.spacing_mut().item_spacing = [0.0, 0.0].into();
                let main_panel_rect = ui.available_rect_before_wrap();
                main_panel_render(ui, state, main_panel_rect, &mut capture_state);
                main_panel_rect
            })
            .inner;

        GuiFrameOutput {
            capture_state,
            main_panel_rect,
        }
    }
}
