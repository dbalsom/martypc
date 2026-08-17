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

//! Common thread events for MartyPC front-ends. These are used to communicate
//! with async operations such as native file dialogs, which operate in async
//! contexts.  They are in the frontend common crate as they need to be shared
//! between the frontend and marty_egui.

use fluxfox::DiskImageFileFormat;
use std::path::{Path, PathBuf};

/// [FileSelectionContext] provides a way to identify how a file was selected. If `Index` is used,
/// then the user selected a particular item from a quick-access file menu where each item has
/// a corresponding index. If `Path` is used, then the user selected a file via a file dialog.
#[derive(Clone, Debug)]
pub enum FileSelectionContext {
    Uninitialized,
    Index(usize),
    Path(PathBuf),
}

impl FileSelectionContext {
    pub fn from_index(index: usize) -> Self {
        FileSelectionContext::Index(index)
    }
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        FileSelectionContext::Path(path.as_ref().to_path_buf())
    }
}

/// [FileOpenContext] provides a way to identify for what purpose a file was loaded.
/// If `FloppyDiskImage` is used, then the file was loaded as a floppy disk image.
/// If `CartridgeImage` is used, then the file was loaded as a PCjr cartridge image.
/// If `VhdDiskImage` is used, then the selected path is opened directly as a writable VHD.
#[derive(Clone, Debug)]
pub enum FileOpenContext {
    ServiceHostFile {
        fsc: FileSelectionContext,
    },
    FloppyDiskImage {
        drive_select: usize,
        fsc: FileSelectionContext,
    },
    CartridgeImage {
        slot_select: usize,
        fsc: FileSelectionContext,
    },
    #[cfg(not(target_arch = "wasm32"))]
    VhdDiskImage {
        drive_select: usize,
    },
}

/// Identifies the purpose of a native directory picker.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub enum DirectoryOpenContext {
    VhdSource,
}

impl FileOpenContext {
    pub fn set_fsc(&mut self, fsc: FileSelectionContext) {
        match self {
            FileOpenContext::ServiceHostFile { fsc: fsc_ref } => {
                *fsc_ref = fsc;
            }
            FileOpenContext::FloppyDiskImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            FileOpenContext::CartridgeImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            #[cfg(not(target_arch = "wasm32"))]
            FileOpenContext::VhdDiskImage { .. } => {}
        }
    }
}

/// [FileSaveContext] provides a way to identify for what purpose a file was saved.
/// If `FloppyDiskImage` is used, then the file was saved as a floppy disk image.
/// If `GuestFile` is used, then the contents were received through a guest file transfer.
/// If `Screenshot` is used, then encoded screenshot data should be written by the file dialog.
/// If `VhdDiskImage` is used, then the selected path will be used to create a VHD.
#[derive(Clone, Debug)]
pub enum FileSaveContext {
    GuestFile {
        filename: String,
        contents: Vec<u8>,
        fsc: FileSelectionContext,
    },
    Screenshot {
        filename: String,
        contents: Vec<u8>,
        fsc: FileSelectionContext,
    },
    FloppyDiskImage {
        drive_select: usize,
        format: DiskImageFileFormat,
        fsc: FileSelectionContext,
    },
    #[cfg(not(target_arch = "wasm32"))]
    VhdDiskImage {
        fsc: FileSelectionContext,
    },
}

impl FileSaveContext {
    pub fn guest_file(filename: impl Into<String>, contents: Vec<u8>) -> Self {
        let filename = filename.into();
        let suggested_filename = filename
            .rsplit(['/', '\\'])
            .find(|component| !component.is_empty())
            .unwrap_or("guest-file.bin")
            .to_string();

        Self::GuestFile {
            filename: suggested_filename,
            contents,
            fsc: FileSelectionContext::Uninitialized,
        }
    }

    pub fn screenshot(filename: impl Into<String>, contents: Vec<u8>) -> Self {
        Self::Screenshot {
            filename: filename.into(),
            contents,
            fsc: FileSelectionContext::Uninitialized,
        }
    }

    pub fn suggested_filename(&self) -> Option<&str> {
        match self {
            FileSaveContext::GuestFile { filename, .. } => Some(filename),
            FileSaveContext::Screenshot { filename, .. } => Some(filename),
            FileSaveContext::FloppyDiskImage { .. } => None,
            #[cfg(not(target_arch = "wasm32"))]
            FileSaveContext::VhdDiskImage { .. } => None,
        }
    }

    pub fn set_fsc(&mut self, fsc: FileSelectionContext) {
        match self {
            FileSaveContext::GuestFile { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            FileSaveContext::Screenshot { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            FileSaveContext::FloppyDiskImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            #[cfg(not(target_arch = "wasm32"))]
            FileSaveContext::VhdDiskImage { fsc: fsc_ref } => {
                *fsc_ref = fsc;
            }
        }
    }

    pub fn take_contents(&mut self) -> Option<Vec<u8>> {
        match self {
            FileSaveContext::GuestFile { contents, .. } | FileSaveContext::Screenshot { contents, .. } => {
                Some(std::mem::take(contents))
            }
            FileSaveContext::FloppyDiskImage { .. } => None,
            #[cfg(not(target_arch = "wasm32"))]
            FileSaveContext::VhdDiskImage { .. } => None,
        }
    }
}

/// An enum representing the various events that can be sent to the frontend via crossbeam upon
/// the completion of an async task. This enum is generic for type D representing a DiskImage.
/// This is usually some sort of container around a fluxfox [DiskImage].
pub enum FrontendThreadEvent<D> {
    FileOpenDialogComplete {
        context: FileOpenContext,
        path: Option<PathBuf>,
        contents: Vec<u8>,
    },
    #[cfg(not(target_arch = "wasm32"))]
    FileOpenPathDialogComplete {
        context: FileOpenContext,
        path:    PathBuf,
    },
    #[cfg(not(target_arch = "wasm32"))]
    DirectoryOpenDialogComplete {
        context: DirectoryOpenContext,
        path:    PathBuf,
    },
    #[cfg(not(target_arch = "wasm32"))]
    DirectoryOpenError(DirectoryOpenContext, String),
    #[cfg(not(target_arch = "wasm32"))]
    DirectoryOpenDialogCancelled(DirectoryOpenContext),
    FileSaveDialogComplete(FileSaveContext),
    FileOpenError(FileOpenContext, String),
    FileSaveError(String),
    FileOpenDialogCancelled(FileOpenContext),
    FileDialogCancelled,
    FloppyImageLoadError(String),
    FloppyImageBeginLongLoad,
    FloppyImageLoadProgress(String, f64),
    FloppyImageLoadComplete {
        drive_select: usize,
        item: FileSelectionContext,
        image: D,
        path: Option<PathBuf>,
    },
    FloppyImageSaveError(String),
    FloppyImageSaveComplete(PathBuf),
    QuitRequested,
    ToggleFullscreen,
}
