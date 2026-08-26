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

    egui::modal.rs

    Implement modal contexts, mostly for handling save/open dialogs.

*/
pub struct ProgressWindow {
    pub title:    String,
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModalMode {
    #[default]
    None,
    FileDialogOpen,
    Notice,
    Progress,
    DragDrop,
}

impl ModalMode {
    pub fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn blocks_workspace(self) -> bool {
        matches!(self, Self::FileDialogOpen | Self::Notice | Self::Progress)
    }
}

#[derive(Default)]
pub enum ModalState {
    #[default]
    None,
    FileDialogOpen {
        message: String,
    },
    Notice(String),
    Progress(ProgressWindow),
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(&self) -> ModalMode {
        match self {
            Self::None => ModalMode::None,
            Self::FileDialogOpen { .. } => ModalMode::FileDialogOpen,
            Self::Notice(_) => ModalMode::Notice,
            Self::Progress(_) => ModalMode::Progress,
        }
    }

    pub fn open_file_dialog(&mut self, message: impl Into<String>) {
        *self = Self::FileDialogOpen {
            message: message.into(),
        };
    }

    pub fn open_notice(&mut self, message: impl Into<String>) {
        *self = Self::Notice(message.into());
    }

    pub fn open_progress(&mut self, title: impl Into<String>, progress: f32) {
        *self = Self::Progress(ProgressWindow {
            title: title.into(),
            progress,
        });
    }

    pub fn close(&mut self) {
        *self = Self::None;
    }

    pub fn close_file_dialog(&mut self) {
        if matches!(self, Self::FileDialogOpen { .. }) {
            self.close();
        }
    }

    pub fn close_progress(&mut self) {
        if matches!(self, Self::Progress(_)) {
            self.close();
        }
    }

    pub fn show(&self, ctx: &egui::Context) {
        match self {
            Self::None => {}
            Self::FileDialogOpen { message } => {
                show_message(ctx, "modal_file_dialog", message);
            }
            Self::Notice(message) => {
                show_message(ctx, "modal_notice", message);
            }
            Self::Progress(progress) => {
                egui::Modal::new(egui::Id::new("modal_progress")).show(ctx, |ui| {
                    ui.set_min_width(400.0);
                    ui.heading(&progress.title);
                    ui.add(
                        egui::ProgressBar::new(progress.progress)
                            .desired_width(ui.available_width())
                            .text(format!("{:.1}%", progress.progress * 100.0)),
                    );
                });
            }
        }
    }
}

fn show_message(ctx: &egui::Context, id: &'static str, message: &str) {
    egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
        ui.label(message);
    });
}

#[cfg(test)]
mod tests {
    use super::{ModalMode, ModalState};

    #[test]
    fn mode_specific_close_does_not_close_another_modal() {
        let mut modal = ModalState::default();

        modal.open_progress("Loading", 0.5);
        modal.close_file_dialog();
        assert_eq!(modal.mode(), ModalMode::Progress);

        modal.open_file_dialog("Choose a file");
        modal.close_progress();
        assert_eq!(modal.mode(), ModalMode::FileDialogOpen);
    }

    #[test]
    fn persistent_modal_modes_are_reported() {
        let mut modal = ModalState::default();
        assert_eq!(modal.mode(), ModalMode::None);

        modal.open_notice("Notice");
        assert_eq!(modal.mode(), ModalMode::Notice);

        modal.close();
        assert_eq!(modal.mode(), ModalMode::None);
    }
}
