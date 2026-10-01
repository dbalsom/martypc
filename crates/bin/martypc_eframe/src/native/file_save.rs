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
*/

use crate::file_transfer::file_transfer_basename;
use marty_core::devices::virtual_printer::{PrinterArtifact, PrinterConversion};
use marty_frontend_common::resource_manager::ResourceManager;
use std::{io::ErrorKind, path::Path, process::Command};

pub(crate) fn save_non_interactive_file(
    resource_manager: &ResourceManager,
    filename: &str,
    data: &[u8],
) -> Result<String, String> {
    let basename = file_transfer_basename(filename)?;
    let path = resource_manager
        .resolve_resource_path_for_write("file_transfer", basename)
        .map_err(|error| error.to_string())?;
    std::fs::write(&path, data).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

pub(crate) fn save_printer_artifact(
    resource_manager: &ResourceManager,
    artifact: &PrinterArtifact,
) -> Result<Vec<String>, String> {
    let page = artifact.page.map_or(String::new(), |page| format!("-page-{page:03}"));
    let stem = format!("print-{:06}{page}", artifact.job_id);
    let mut copy = 1;
    let path = loop {
        let suffix = if copy == 1 { String::new() } else { format!("-{copy}") };
        let filename = format!("{stem}{suffix}.{}", artifact.extension);
        let candidate = resource_manager
            .resolve_resource_path_for_write("printer", filename)
            .map_err(|error| error.to_string())?;
        let pdf_exists = artifact.conversion.is_some() && candidate.with_extension("pdf").exists();
        if !candidate.exists() && !pdf_exists {
            break candidate;
        }
        copy += 1;
    };
    std::fs::write(&path, &artifact.data).map_err(|error| error.to_string())?;

    let mut saved = vec![path.to_string_lossy().into_owned()];
    if let Some(conversion) = artifact.conversion {
        let pdf_path = path.with_extension("pdf");
        convert_to_pdf(conversion, &path, &pdf_path).map_err(|error| {
            format!(
                "Saved source output to '{}', but PDF conversion failed: {error}",
                path.display()
            )
        })?;
        saved.push(pdf_path.to_string_lossy().into_owned());
    }
    Ok(saved)
}

fn convert_to_pdf(conversion: PrinterConversion, source: &Path, destination: &Path) -> Result<(), String> {
    let programs: &[&str] = match conversion {
        PrinterConversion::PostscriptToPdf => &["gs", "gswin64c", "gswin32c"],
        PrinterConversion::PclToPdf => &["gpcl6", "gpcl6win64", "gpcl6win32"],
    };
    for program in programs {
        match Command::new(program)
            .arg("-dBATCH")
            .arg("-dNOPAUSE")
            .arg("-sDEVICE=pdfwrite")
            .arg(format!("-sOutputFile={}", destination.display()))
            .arg(source)
            .status()
        {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => return Err(format!("{program} exited with status {status}")),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Failed to run {program}: {error}")),
        }
    }
    Err(format!(
        "None of these converters were found on PATH: {}",
        programs.join(", ")
    ))
}
