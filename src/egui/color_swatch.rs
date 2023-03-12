use crate::egui::GuiState;
use egui::*;

impl GuiState {

    pub fn color_swatch(ui: &mut Ui, color: Color32, open: bool) -> Response {
        let size = ui.spacing().interact_size;
        let size = egui::Vec2 { x: size.y, y: size.y}; // Make square
        let (rect, response) = ui.allocate_exact_size(size, Sense { click: false, drag: false, focusable: false});
        //response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));

        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);

        if ui.is_rect_visible(rect) {
            let visuals = if open {
                &ui.visuals().widgets.open
            } else {
                ui.style().interact(&response)
            };
            //let rect = rect.expand(visuals.expansion);

            //painter.rect_filled(rect, 0.0, color);

            let rounding = visuals.rounding.at_most(2.0);

            ui.painter().rect_filled(rect, rounding, color);
            ui.painter().rect_stroke(rect, rounding, (2.0, visuals.bg_fill)); // fill is intentional, because default style has no border
        }

        response
    }
}