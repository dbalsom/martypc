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

    marty_egui::widgets::sector_status.rs

    Implements a custom control that displays a sector status indicator.

*/

use egui::*;
use fluxfox::SectorMapEntry;

const COLOR_SECTOR_OK: Color32 = Color32::from_rgb(0, 255, 0);
const COLOR_BAD_CRC: Color32 = Color32::from_rgb(0xef, 0x7d, 0x57);
const COLOR_DELETED_DATA: Color32 = Color32::from_rgb(0x25, 0x71, 0x79);
#[allow(dead_code)]
const COLOR_BAD_DELETED_DATA: Color32 = Color32::from_rgb(0xb1, 0x3e, 0x53);
const COLOR_BAD_HEADER: Color32 = Color32::RED;
const COLOR_NO_DAM: Color32 = Color32::GRAY;

/// Simple color swatch widget. Used for palette register display.
pub fn sector_status(ui: &mut Ui, entry: &SectorMapEntry, open: bool) -> Response {
    let size = ui.spacing().interact_size;
    let size = egui::Vec2 { x: size.y, y: size.y }; // Make square
    let (rect, response) = ui.allocate_exact_size(
        size,
        Sense {
            click: false,
            drag: false,
            focusable: false,
        },
    );
    //response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));

    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);

    if ui.is_rect_visible(rect) {
        let visuals = if open {
            &ui.visuals().widgets.open
        }
        else {
            ui.style().interact(&response)
        };
        //let rect = rect.expand(visuals.expansion);

        //painter.rect_filled(rect, 0.0, color);

        let rounding = visuals.rounding.at_most(2.0);

        // pub chsn: DiskChsn,
        // pub address_crc_valid: bool,
        // pub data_crc_valid: bool,
        // pub deleted_mark: bool,
        // pub no_dam: bool,
        let color = match (
            entry.attributes.address_error,
            entry.attributes.data_error,
            entry.attributes.deleted_mark,
            entry.attributes.no_dam,
        ) {
            (false, false, false, false) => COLOR_SECTOR_OK,
            (false, false, true, _) => COLOR_DELETED_DATA,
            (false, true, _, _) => COLOR_BAD_CRC,
            (false, false, false, true) => COLOR_NO_DAM,
            (true, _, _, _) => COLOR_BAD_HEADER,
        };

        ui.painter().rect_filled(rect, rounding, color);
        ui.painter().rect_stroke(rect, rounding, (2.0, visuals.bg_fill)); // fill is intentional, because default style has no border
    }

    // Add hover UI
    response.on_hover_ui(|ui| {
        Grid::new("popup_sector_attributes_grid").show(ui, |ui| {
            ui.label("ID:");
            ui.label(entry.chsn.to_string());
            ui.end_row();

            ui.label("Size:");
            ui.label(entry.chsn.n_size().to_string());
            ui.end_row();

            let good_color = ui.visuals().text_color();
            let bad_color = ui.visuals().warn_fg_color;

            match entry.attributes.address_error {
                true => ui.colored_label(bad_color, "Address CRC is invalid"),
                false => ui.colored_label(good_color, "Address CRC is valid"),
            };
            ui.end_row();

            match entry.attributes.data_error {
                true => ui.colored_label(bad_color, "Data CRC is invalid"),
                false => ui.colored_label(good_color, "Data CRC is valid"),
            };
            ui.end_row();

            match (entry.attributes.no_dam, entry.attributes.deleted_mark) {
                (true, _) => ui.colored_label(bad_color, "Sector has no data"),
                (false, true) => ui.colored_label(bad_color, "Deleted data marker"),
                (false, false) => ui.colored_label(good_color, "Normal data marker"),
            };
            ui.end_row();
        });
    })
}
