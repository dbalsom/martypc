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
use std::fmt::{Display, Formatter};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
const PORTABLE_MARKER_FILENAME: &str = "portable.txt";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeploymentState {
    Installed,
    Portable,
    Web,
}

impl DeploymentState {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn detect() -> Self {
        match std::env::current_exe() {
            Ok(executable_path) => {
                if let Some(executable_dir) = executable_path.parent() {
                    if Self::portable_marker_exists(executable_dir) {
                        return Self::Portable;
                    }
                }
                else {
                    log::warn!(
                        "Executable path has no parent directory while detecting deployment state: {}",
                        executable_path.display()
                    );
                }
            }
            Err(err) => {
                log::warn!("Failed to determine executable path while detecting deployment state: {err}");
            }
        }

        match std::env::current_dir() {
            Ok(current_dir) if Self::portable_marker_exists(&current_dir) => Self::Portable,
            Ok(_) => Self::Installed,
            Err(err) => {
                log::warn!("Failed to determine current directory while detecting deployment state: {err}");
                Self::Installed
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn detect() -> Self {
        Self::Web
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn portable_marker_exists(directory: &Path) -> bool {
        directory.join(PORTABLE_MARKER_FILENAME).is_file()
    }
}

impl Default for DeploymentState {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::Installed
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self::Web
        }
    }
}

impl Display for DeploymentState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installed => f.write_str("MartyPC"),
            Self::Portable => f.write_str("MartyPC Portable"),
            Self::Web => f.write_str("MartyPC Web Edition"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::{fs, time::SystemTime};

    use super::*;

    #[test]
    fn deployment_state_has_application_title_display() {
        assert_eq!(DeploymentState::Installed.to_string(), "MartyPC");
        assert_eq!(DeploymentState::Portable.to_string(), "MartyPC Portable");
        assert_eq!(DeploymentState::Web.to_string(), "MartyPC Web Edition");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn portable_marker_is_detected_in_either_search_directory() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock should be later than the Unix epoch")
            .as_nanos();
        let test_root =
            std::env::temp_dir().join(format!("martypc-deployment-state-test-{}-{unique}", std::process::id()));
        let executable_dir = test_root.join("bin");
        let working_dir = test_root.join("work");
        fs::create_dir_all(&executable_dir).expect("temporary executable directory should be created");
        fs::create_dir(&working_dir).expect("temporary working directory should be created");

        assert!(!DeploymentState::portable_marker_exists(&executable_dir));
        assert!(!DeploymentState::portable_marker_exists(&working_dir));

        fs::write(executable_dir.join(PORTABLE_MARKER_FILENAME), [])
            .expect("executable-directory portable marker should be created");
        assert!(DeploymentState::portable_marker_exists(&executable_dir));

        fs::remove_file(executable_dir.join(PORTABLE_MARKER_FILENAME))
            .expect("executable-directory portable marker should be removed");
        fs::write(working_dir.join(PORTABLE_MARKER_FILENAME), [])
            .expect("working-directory portable marker should be created");
        assert!(DeploymentState::portable_marker_exists(&working_dir));

        fs::remove_dir_all(test_root).expect("temporary test directory should be removed");
    }
}
