/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2025 Daniel Balsom

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

use std::sync::Arc;

use marty_display_common::display_manager::DmGuiOptions;
use marty_egui::{
    state::GuiState,
    themes::{make_theme, GuiTheme},
};
use marty_frontend_common::{GuiContextOptions, MartyGuiTheme};

use egui::{Color32, Context};
use egui_extras::install_image_loaders;

//use web_time::{Duration, Instant};

/// Manages all state required for rendering egui over `Pixels`.
#[allow(dead_code)]
pub struct GuiRenderContext {
    /// Cloned egui context, in case we need to access it.
    ctx: Context,
    /// The theme to use for the main UI.
    main_theme: Arc<dyn GuiTheme>,
    /// The theme to use for the menu UI.
    menu_theme: Arc<dyn GuiTheme>,
    /// The global scale factor for the UI.
    scale_factor: f64,
}

impl Default for GuiRenderContext {
    fn default() -> Self {
        let ctx = Context::default();
        let main_theme = make_theme(MartyGuiTheme::default());
        let menu_theme = make_theme(MartyGuiTheme::default());
        Self {
            ctx,
            main_theme,
            menu_theme,
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
        let main_theme = make_theme(gui_theme_enum);
        let menu_theme = make_theme(menu_theme_enum);

        // Make header smaller, regardless of theme.
        use egui::{FontFamily::Proportional, FontId, TextStyle::*};
        let mut style = (*ctx.style()).clone();

        style.text_styles.entry(Heading).and_modify(|text_style| {
            *text_style = FontId::new(14.0, Proportional);
        });
        ctx.set_style(style);
        ctx.set_visuals(main_theme.visuals());

        #[cfg(debug_assertions)]
        if gui_options.debug_drawing {
            ctx.set_debug_on_hover(true);
        }

        let slf = Self {
            ctx,
            main_theme,
            menu_theme,
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

    pub fn show<Fw, Fm>(
        &mut self,
        state: &mut GuiState,
        show_menu: bool,
        main_panel_fill: Option<Color32>,
        mut window_render: Fw,
        mut main_panel_render: Fm,
    ) -> Option<bool>
    where
        Fw: FnMut(&mut egui::Context, &mut GuiState, &mut Option<bool>),
        Fm: FnMut(&mut egui::Ui, &mut GuiState, f32, &mut Option<bool>),
    {
        let mut capture_state = None;

        if show_menu {
            self.ctx.set_visuals(self.menu_theme.visuals());
            egui::TopBottomPanel::top("martypc_top_panel").show(&self.ctx, |ui| {
                state.show_menu(ui);
            });
        }

        self.ctx.set_visuals(self.main_theme.visuals());
        state.show_windows(&self.ctx);

        let old_margin = self.ctx.style().spacing.window_margin;
        // Disable window margin for display window.
        self.ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::ZERO;
        });
        window_render(&mut self.ctx, state, &mut capture_state);
        // Restore window margin.
        self.ctx.style_mut(|style| {
            style.spacing.window_margin = old_margin;
        });

        // Override panel fill if requested.
        let mut panel_frame = egui::Frame::default();
        panel_frame.inner_margin = egui::Margin::ZERO;
        panel_frame.fill = self.main_theme.visuals().panel_fill;
        if let Some(fill) = main_panel_fill {
            panel_frame.fill = fill;
        }
        egui::CentralPanel::default().frame(panel_frame).show(&self.ctx, |ui| {
            ui.spacing_mut().item_spacing = [0.0, 0.0].into();
            let menu_height = self.ctx.screen_rect().size().y - ui.available_size().y;
            main_panel_render(ui, state, menu_height, &mut capture_state);
        });

        capture_state
    }
}
