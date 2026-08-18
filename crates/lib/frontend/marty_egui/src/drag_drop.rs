/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    egui::src::drag_drop.rs

    Drag-and-drop destination selection.
*/

use std::path::PathBuf;

use egui::{DroppedFile, RichText, Sense, Stroke, StrokeKind};
use marty_core::machine_types::FloppyDriveType;
use marty_frontend_common::{
    exec_async,
    thread_events::{FileOpenContext, FileSelectionContext, FrontendThreadEvent},
};

use crate::state::GuiState;

const DROP_TARGET_SIZE: f32 = 144.0;
const DROP_TARGET_SPACING: f32 = 10.0;
const DROP_TARGET_IMAGE_SIZE: f32 = 88.0;
const DROP_TARGET_IMAGE_TOP_MARGIN: f32 = 8.0;
const DROP_TARGET_LABEL_BOTTOM_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DropTarget {
    FloppyDrive(usize),
}

impl GuiState {
    pub(crate) fn drag_drop_active(ctx: &egui::Context) -> bool {
        ctx.input(|input| !input.raw.hovered_files.is_empty() || !input.raw.dropped_files.is_empty())
    }

    pub(crate) fn show_drag_drop_modal(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());

        let targets: Vec<_> = self
            .floppy_drives
            .iter()
            .enumerate()
            .map(|(drive_idx, drive)| {
                let drive_letter = u8::try_from(drive_idx)
                    .ok()
                    .and_then(|idx| b'A'.checked_add(idx))
                    .map(char::from);
                let drive_name = drive_letter.map_or_else(
                    || format!("Floppy Drive {drive_idx}"),
                    |letter| format!("Floppy Drive {drive_idx} ({letter}:)"),
                );

                (
                    DropTarget::FloppyDrive(drive_idx),
                    format!("{drive_name}\n{}", drive.drive_type),
                    drive.drive_type,
                )
            })
            .collect();

        let column_count = smallest_square_width(targets.len());
        let pointer_pos = drag_pointer_pos(ctx);
        let mut hovered_target = None;

        egui::Modal::new(egui::Id::new("drag_drop_target_modal")).show(ctx, |ui| {
            ui.heading("Drop File(s)");
            ui.label("Choose what MartyPC should do with the dropped file.");
            ui.add_space(8.0);

            if targets.is_empty() {
                ui.label("No drop targets are currently available.");
                return;
            }

            egui::Grid::new("drag_drop_target_grid")
                .num_columns(column_count)
                .spacing(egui::vec2(DROP_TARGET_SPACING, DROP_TARGET_SPACING))
                .show(ui, |ui| {
                    for (target_idx, (target, label, drive_type)) in targets.iter().enumerate() {
                        if target_idx > 0 && target_idx % column_count == 0 {
                            ui.end_row();
                        }

                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(DROP_TARGET_SIZE, DROP_TARGET_SIZE), Sense::hover());
                        // An external OS drag is not an egui drag, so Response::hovered() may be
                        // suppressed by the modal's interaction layer. Hit-test the last known
                        // pointer position directly against each destination instead.
                        let is_hovered = pointer_pos.is_some_and(|pointer_pos| rect.contains(pointer_pos));
                        let visuals = ui.visuals();
                        let fill = if is_hovered {
                            visuals.selection.bg_fill
                        }
                        else {
                            visuals.widgets.inactive.bg_fill
                        };
                        let stroke = if is_hovered {
                            Stroke::new(3.0, visuals.selection.stroke.color)
                        }
                        else {
                            Stroke::new(1.0, visuals.widgets.inactive.bg_stroke.color)
                        };

                        ui.painter().rect(rect, 8.0, fill, stroke, StrokeKind::Inside);
                        let image_rect = egui::Rect::from_center_size(
                            egui::pos2(
                                rect.center().x,
                                rect.top() + DROP_TARGET_IMAGE_TOP_MARGIN + DROP_TARGET_IMAGE_SIZE / 2.0,
                            ),
                            egui::Vec2::splat(DROP_TARGET_IMAGE_SIZE),
                        );
                        egui::Image::new(floppy_artwork(*drive_type))
                            .texture_options(egui::TextureOptions::NEAREST)
                            .paint_at(ui, image_rect);
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - DROP_TARGET_LABEL_BOTTOM_MARGIN),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(16.0),
                            if is_hovered {
                                visuals.selection.stroke.color
                            }
                            else {
                                visuals.widgets.inactive.fg_stroke.color
                            },
                        );

                        if is_hovered {
                            hovered_target = Some(*target);
                        }
                    }
                });

            ui.add_space(8.0);
            ui.label(RichText::new("Release the file over a destination.").weak());
        });

        if dropped_files.is_empty() {
            return;
        }

        if dropped_files.len() != 1 {
            self.toasts.error("Drop one file at a time.");
            return;
        }

        let Some(target) = hovered_target
        else {
            return;
        };

        match target {
            DropTarget::FloppyDrive(drive_idx) => {
                self.load_dropped_floppy(drive_idx, dropped_files.into_iter().next().unwrap());
            }
        }
    }

    fn load_dropped_floppy(&mut self, drive_idx: usize, dropped_file: DroppedFile) {
        let source_path = dropped_file.path;
        let selection_path = source_path
            .clone()
            .or_else(|| (!dropped_file.name.is_empty()).then(|| PathBuf::from(dropped_file.name.clone())));

        let context = FileOpenContext::FloppyDiskImage {
            drive_select: drive_idx,
            fsc: selection_path
                .clone()
                .map(FileSelectionContext::Path)
                .unwrap_or(FileSelectionContext::Uninitialized),
        };

        exec_async(self.thread_sender.clone(), async move {
            if let Some(contents) = dropped_file.bytes {
                return FrontendThreadEvent::FileOpenDialogComplete {
                    context,
                    path: selection_path,
                    contents: contents.to_vec(),
                };
            }

            let Some(path) = source_path
            else {
                return FrontendThreadEvent::FileOpenError(
                    context,
                    "The dropped file did not provide a path or file contents.".to_string(),
                );
            };

            match std::fs::read(&path) {
                Ok(contents) => FrontendThreadEvent::FileOpenDialogComplete {
                    context,
                    path: Some(path),
                    contents,
                },
                Err(error) => FrontendThreadEvent::FileOpenError(context, error.to_string()),
            }
        });
    }
}

fn floppy_artwork(drive_type: FloppyDriveType) -> egui::ImageSource<'static> {
    match drive_type {
        FloppyDriveType::Floppy360K => {
            egui::include_image!("../../../../../assets/5_25_dd_floppy.png")
        }
        FloppyDriveType::Floppy720K => {
            egui::include_image!("../../../../../assets/3_5_dd_floppy.png")
        }
        FloppyDriveType::Floppy12M => {
            egui::include_image!("../../../../../assets/5_25_hd_floppy.png")
        }
        FloppyDriveType::Floppy144M => {
            egui::include_image!("../../../../../assets/3_5_hd_floppy.png")
        }
    }
}

fn smallest_square_width(target_count: usize) -> usize {
    if target_count == 0 {
        return 1;
    }

    let mut width = 1;
    while width * width < target_count {
        width += 1;
    }
    width
}

fn drag_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    #[cfg(target_os = "windows")]
    if let Some(pointer_pos) = windows_drag_pointer_pos(ctx) {
        return Some(pointer_pos);
    }

    ctx.pointer_hover_pos()
}

/// Ugly hack to get the pointer position during a drag event in Windows. Winit doesn't expose this
/// through its API. I really wanted to see the drag n' drop targets highlight when you hover over
/// them, so here we are doing raw winapi calls in the middle of everything. Gross.
#[cfg(target_os = "windows")]
fn windows_drag_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

    let mut cursor_pos = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut cursor_pos) } == 0 {
        return None;
    }

    let viewport_origin = ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.min))?;
    let pixels_per_point = ctx.pixels_per_point();
    let screen_pos = egui::pos2(
        cursor_pos.x as f32 / pixels_per_point,
        cursor_pos.y as f32 / pixels_per_point,
    );

    Some(screen_pos - viewport_origin.to_vec2())
}

#[cfg(test)]
mod tests {
    use super::smallest_square_width;

    #[test]
    fn drop_target_grid_uses_smallest_fitting_square() {
        assert_eq!(smallest_square_width(0), 1);
        assert_eq!(smallest_square_width(1), 1);
        assert_eq!(smallest_square_width(2), 2);
        assert_eq!(smallest_square_width(4), 2);
        assert_eq!(smallest_square_width(5), 3);
        assert_eq!(smallest_square_width(9), 3);
    }
}
