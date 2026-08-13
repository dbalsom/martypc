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

//! Wrappers for handling file dialogs for the MartyPC emulator's egui interface.

use crate::{state::GuiState, DialogProvider};

use marty_frontend_common::{
    exec_async,
    thread_events::{FileOpenContext, FileSaveContext, FileSelectionContext, FrontendThreadEvent},
};
#[cfg(feature = "use_rfd")]
use rfd;

pub struct FileDialogFilter {
    pub desc: String,
    pub extensions: Vec<String>,
}

impl FileDialogFilter {
    pub fn new(desc: impl Into<String>, extensions: Vec<impl Into<String>>) -> Self {
        Self {
            desc: desc.into(),
            extensions: extensions.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl GuiState {
    /// Open a file picker. If `read_file` is true, read the selected file into memory; otherwise,
    /// return only its native filesystem path so the caller can retain direct file access.
    pub fn open_file_dialog(
        &mut self,
        context: FileOpenContext,
        title: impl AsRef<str>,
        filters: Vec<FileDialogFilter>,
        read_file: bool,
    ) {
        match self.dialog_provider {
            DialogProvider::EguiFileDialog => {
                log::warn!("egui-file-dialog not implemented");
                let _ = self.thread_sender.send(FrontendThreadEvent::FileOpenError(
                    context,
                    "egui-file-dialog not implemented".to_string(),
                ));
            }
            #[cfg(feature = "use_rfd")]
            DialogProvider::Rfd => {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title.as_ref());

                for filter in filters {
                    dialog = dialog.add_filter(filter.desc, &filter.extensions);
                }
                let task = dialog.pick_file();
                exec_async(self.thread_sender.clone(), async move {
                    let mut resolved_context = context;
                    let rfd_handle = task.await;

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        return if let Some(file_handle) = rfd_handle {
                            let path_buf = file_handle.path().to_path_buf();
                            resolved_context.set_fsc(FileSelectionContext::Path(path_buf.clone()));

                            if read_file {
                                match std::fs::read(&path_buf) {
                                    Ok(vec) => FrontendThreadEvent::FileOpenDialogComplete {
                                        context: resolved_context,
                                        path: Some(path_buf),
                                        contents: vec,
                                    },
                                    Err(e) => FrontendThreadEvent::FileOpenError(resolved_context, e.to_string()),
                                }
                            }
                            else {
                                FrontendThreadEvent::FileOpenPathDialogComplete {
                                    context: resolved_context,
                                    path:    path_buf,
                                }
                            }
                        }
                        else {
                            FrontendThreadEvent::FileOpenDialogCancelled(resolved_context)
                        };
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        use std::path::PathBuf;
                        return if let Some(file_handle) = rfd_handle {
                            let file_name = file_handle.file_name().to_string();
                            resolved_context.set_fsc(FileSelectionContext::Path(PathBuf::from(file_name.clone())));

                            if read_file {
                                FrontendThreadEvent::FileOpenDialogComplete {
                                    context: resolved_context,
                                    path: None, // No path available on WASM
                                    contents: file_handle.read().await,
                                }
                            }
                            else {
                                FrontendThreadEvent::FileOpenError(
                                    resolved_context,
                                    "Path-only file dialogs are not supported on WASM".to_string(),
                                )
                            }
                        }
                        else {
                            FrontendThreadEvent::FileOpenDialogCancelled(resolved_context)
                        };
                    }
                });
            }
        }
    }

    pub fn save_file_dialog(&self, context: FileSaveContext, title: impl AsRef<str>, filters: Vec<FileDialogFilter>) {
        match self.dialog_provider {
            DialogProvider::EguiFileDialog => {
                log::warn!("egui-file-dialog not implemented");
                let _ = self
                    .thread_sender
                    .send(FrontendThreadEvent::FileSaveError("egui-file-dialog not implemented".to_string()));
            }
            #[cfg(feature = "use_rfd")]
            DialogProvider::Rfd => {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title.as_ref());

                if let Some(filename) = context.suggested_filename() {
                    dialog = dialog.set_file_name(filename);
                }

                for filter in filters {
                    dialog = dialog.add_filter(filter.desc, &filter.extensions);
                }
                let task = dialog.save_file();
                exec_async(self.thread_sender.clone(), async move {
                    let Some(file_handle) = task.await
                    else {
                        return FrontendThreadEvent::FileDialogCancelled;
                    };

                    #[cfg(not(target_arch = "wasm32"))]
                    let fsc = FileSelectionContext::Path(file_handle.path().to_path_buf());

                    #[cfg(target_arch = "wasm32")]
                    let fsc = FileSelectionContext::Path(std::path::PathBuf::from(file_handle.file_name()));

                    let mut resolved_context = context;
                    resolved_context.set_fsc(fsc);

                    if let FileSaveContext::GuestFile { contents, .. } = &mut resolved_context {
                        let contents = std::mem::take(contents);
                        if let Err(error) = file_handle.write(&contents).await {
                            return FrontendThreadEvent::FileSaveError(error.to_string());
                        }
                    }

                    FrontendThreadEvent::FileSaveDialogComplete(resolved_context)
                });
            }
        }
    }
}
