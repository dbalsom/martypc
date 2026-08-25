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

//! Build procedure for MartyPC.
//! This build script is used to compile the Windows icon resource for the executable.

use std::{env, fs, io, path::PathBuf};
use winres::WindowsResource;

fn main() -> io::Result<()> {
    built::write_built_file()?;
    emit_git_rerun_hints();

    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // Create an icon resource for the Windows build.
        // This icon is only used when viewing the executable itself in explorer.
        // We have to set the icon again in Winit for each window we create.
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("../../../assets/martypc.ico")
            .compile()?;
    }
    Ok(())
}

fn emit_git_rerun_hints() {
    let Some(git_dir) = git_dir()
    else {
        return;
    };

    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());
    println!("cargo:rerun-if-changed={}", git_dir.join("packed-refs").display());

    if let Ok(head) = fs::read_to_string(&head_path) {
        if let Some(ref_name) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed={}", git_dir.join(ref_name.trim()).display());
        }
    }
}

fn git_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir.join("../../..").canonicalize().ok()?;
    let git_path = repo_root.join(".git");

    if git_path.is_dir() {
        return Some(git_path);
    }

    let git_file = fs::read_to_string(&git_path).ok()?;
    let git_dir = git_file.strip_prefix("gitdir: ")?.trim();
    let git_dir = PathBuf::from(git_dir);
    Some(if git_dir.is_absolute() {
        git_dir
    }
    else {
        repo_root.join(git_dir)
    })
}
