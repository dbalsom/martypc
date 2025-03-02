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
#[derive(Clone, Debug)]
pub enum FileOpenContext {
    FloppyDiskImage { drive_select: usize, fsc: FileSelectionContext },
    CartridgeImage { slot_select: usize, fsc: FileSelectionContext },
}

impl FileOpenContext {
    pub fn set_fsc(&mut self, fsc: FileSelectionContext) {
        match self {
            FileOpenContext::FloppyDiskImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
            FileOpenContext::CartridgeImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
        }
    }
}

/// [FileSaveContext] provides a way to identify for what purpose a file was saved.
/// If `FloppyDiskImage` is used, then the file was saved as a floppy disk image.
#[derive(Clone, Debug)]
pub enum FileSaveContext {
    FloppyDiskImage {
        drive_select: usize,
        format: DiskImageFileFormat,
        fsc: FileSelectionContext,
    },
}

impl FileSaveContext {
    pub fn set_fsc(&mut self, fsc: FileSelectionContext) {
        match self {
            FileSaveContext::FloppyDiskImage { fsc: fsc_ref, .. } => {
                *fsc_ref = fsc;
            }
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
    FileSaveDialogComplete(FileSaveContext),
    FileOpenError(FileOpenContext, String),
    FileSaveError(String),
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
}
