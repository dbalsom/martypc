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

    egui::modal.rs

    Implement modal contexts, mostly for handling save/open dialogs.

*/
use crate::{GuiEventQueue, PathBuf};
use fluxfox::DiskImageFileFormat;

pub enum ModalContext {
    Notice(String),                                           // Non-interactive dialog with message
    SaveFloppyImage(usize, DiskImageFileFormat, Vec<String>), // Index of the floppy drive, list of extensions
    OpenFloppyImage(usize, Vec<String>),                      // Index of the floppy drive, list of extensions
    ProgressBar(String, f32),                                 // Progress bar with message and progress
}

pub struct ProgressWindow {
    pub title:    String,
    pub progress: f32,
}

pub enum ModalDialog {
    Notice(String),
    // Save(FileDialog),
    // Open(FileDialog),
    ProgressBar(ProgressWindow),
}

#[derive(Default)]
pub struct ModalState {
    pub context: Option<ModalContext>,
    pub dialog: Option<ModalDialog>,
    pub selected_path: Option<PathBuf>,
    pub extensions: Vec<String>,
    pub default_floppy_path: Option<PathBuf>,
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_paths(&mut self, floppy_path: PathBuf) {
        self.default_floppy_path = Some(floppy_path);
    }

    pub fn is_open(&self) -> bool {
        self.context.is_some()
    }

    pub fn open(&mut self, context: ModalContext) {
        match &context {
            ModalContext::Notice(msg) => {
                self.dialog = Some(ModalDialog::Notice(msg.clone()));
            }
            ModalContext::SaveFloppyImage(_, _, _exts) => {}
            ModalContext::OpenFloppyImage(_, _exts) => {}
            ModalContext::ProgressBar(title, progress) => {
                self.dialog = Some(ModalDialog::ProgressBar(ProgressWindow {
                    title:    title.clone(),
                    progress: *progress,
                }));
            }
        }
        self.context = Some(context);
    }

    pub fn close(&mut self) {
        self.context = None;
        self.dialog = None;
        self.selected_path = None;
        self.extensions.clear();
    }

    pub fn show(&mut self, ctx: &egui::Context, _events: &mut GuiEventQueue) {
        match &mut self.dialog {
            // Some(ModalDialog::Save(dialog)) | Some(ModalDialog::Open(dialog)) => {
            //     if dialog.show(ctx).selected() {
            //         if let Some(path) = dialog.path() {
            //             self.selected_path = Some(path.to_path_buf());
            //             //log::warn!("Selected dialog path: {:?}", &self.selected_path.as_ref().unwrap());
            //             dialog_resolved = true;
            //         }
            //     }
            //
            //     if matches!(dialog.state(), egui_file::State::Cancelled | egui_file::State::Closed) {
            //         self.selected_path = None;
            //         dialog_resolved = true;
            //     }
            //
            //     if dialog_resolved {
            //         if let Some(path) = &self.selected_path {
            //             log::warn!("Selected dialog path: {:?}", path);
            //             self.resolve(events);
            //         }
            //
            //         self.context = None;
            //         self.dialog = None;
            //         self.extensions.clear();
            //     }
            //     //log::warn!("dialog state: {:?}", dialog.state());
            // }
            Some(ModalDialog::Notice(msg)) => {
                let id = egui::Id::new("modal_notice");
                let modal = egui::Modal::new(id);

                modal.show(ctx, |ui| {
                    let label_text = msg.clone();
                    ui.label(label_text);
                });
            }
            Some(ModalDialog::ProgressBar(progress)) => {
                egui::Window::new(progress.title.clone())
                    .default_size(egui::vec2(400.0, 100.0))
                    .show(ctx, |ui| {
                        ui.add(
                            egui::ProgressBar::new(progress.progress as f32)
                                .text(format!("{:.1}%", progress.progress * 100.0)),
                        );
                    });
            }
            None => {}
        }
    }

    /*
    fn resolve(&mut self, event_queue: &mut GuiEventQueue) {
        if let Some(context) = &self.context {
            match context {
                ModalContext::Notice(_) => {
                    // Nothing to do to resolve a Notice
                }
                ModalContext::SaveFloppyImage(drive_idx, format, _) => {
                    if let Some(path) = &self.selected_path {
                        log::debug!("ModalState::resolve(): Sending SaveFloppyAs event for drive {} with format {:?} and path {:?}", drive_idx, format, path);
                        event_queue.send(GuiEvent::SaveFloppyAs(*drive_idx, *format, path.clone()));
                    }
                }
                ModalContext::OpenFloppyImage(drive_idx, _) => {
                    if let Some(path) = &self.selected_path {
                        log::debug!(
                            "ModalState::resolve(): Sending OpenFloppyFrom event for drive {} with path {:?}",
                            drive_idx,
                            path
                        );
                        event_queue.send(GuiEvent::LoadFloppyAs(*drive_idx, path.clone()));
                    }
                }
                ModalContext::ProgressBar(_, _) => {
                    // Nothing to do to resolve a ProgressBar
                }
            }
        }

        self.context = None;
        self.dialog = None;
        self.extensions.clear();
    }*/
}
