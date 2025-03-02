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
    pub fn open_file_dialog(
        &mut self,
        context: FileOpenContext,
        title: impl AsRef<str>,
        filters: Vec<FileDialogFilter>,
    ) {
        match self.dialog_provider {
            DialogProvider::EguiFileDialog => {
                log::warn!("egui-file-dialog not implemented");
            }
            #[cfg(feature = "use_rfd")]
            DialogProvider::Rfd => {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title.as_ref());

                for filter in filters {
                    dialog = dialog.add_filter(filter.desc, &filter.extensions);
                }
                let task = dialog.pick_file();
                exec_async(self.thread_sender.clone(), async {
                    let mut resolved_context = context;
                    let rfd_handle = task.await;

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        return if let Some(file_handle) = rfd_handle {
                            let path_buf = file_handle.path().to_path_buf();
                            resolved_context.set_fsc(FileSelectionContext::Path(path_buf.clone()));

                            // Load the file
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
                            FrontendThreadEvent::FileDialogCancelled
                        };
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        use std::path::PathBuf;
                        return if let Some(file_handle) = rfd_handle {
                            let file_name = file_handle.file_name().to_string();
                            resolved_context.set_fsc(FileSelectionContext::Path(PathBuf::from(file_name.clone())));

                            // Read file contents asynchronously
                            FrontendThreadEvent::FileOpenDialogComplete {
                                context: resolved_context,
                                path: None, // No path available on WASM
                                contents: file_handle.read().await,
                            }
                        }
                        else {
                            FrontendThreadEvent::FileDialogCancelled
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
            }
            #[cfg(feature = "use_rfd")]
            DialogProvider::Rfd => {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title.as_ref());

                for filter in filters {
                    dialog = dialog.add_filter(filter.desc, &filter.extensions);
                }
                let task = dialog.save_file();
                exec_async(self.thread_sender.clone(), async {
                    let rfd_handle = task.await;

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        return if let Some(file_handle) = rfd_handle {
                            let path_buf = file_handle.path().to_path_buf();
                            let fsc = FileSelectionContext::Path(path_buf.clone());

                            let mut new_context = context;
                            new_context.set_fsc(fsc);

                            FrontendThreadEvent::FileSaveDialogComplete(new_context)
                        }
                        else {
                            FrontendThreadEvent::FileDialogCancelled
                        };
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        use std::path::PathBuf;
                        return if let Some(file_handle) = rfd_handle {
                            let file_name = file_handle.file_name().to_string();
                            let fsc = FileSelectionContext::Path(PathBuf::from(file_name.clone()));

                            let mut new_context = context;
                            new_context.set_fsc(fsc);

                            FrontendThreadEvent::FileSaveDialogComplete(new_context)
                        }
                        else {
                            FrontendThreadEvent::FileDialogCancelled
                        };
                    }
                });
            }
        }
    }
}
