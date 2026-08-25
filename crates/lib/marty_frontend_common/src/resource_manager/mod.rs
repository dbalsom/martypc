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

    frontend_common::resource_manager::mod.rs

    File and path services for frontends. File operations are abstracted
    to support both local and web filesystems (for wasm compilation).

    Eventually archive support will be added as well.

*/

mod archive_overlay;
#[cfg(not(target_arch = "wasm32"))]
mod local_fs;
mod manifest;
mod path_manager;
pub mod tree;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    future::Future,
    path::{Component, Path, PathBuf},
};

#[cfg(target_arch = "wasm32")]
use crate::resource_manager::manifest::{ManifestFile, ResourceManifest};

use crate::{resource_manager::archive_overlay::ArchiveOverlay, types::resource_location::ResourceLocation};

#[cfg(target_arch = "wasm32")]
use marty_web_helpers::fetch_file;
pub use path_manager::PathConfigItem;
use path_manager::PathManager;

use anyhow::Error;
use regex::Regex;
pub use tree::TreeNode as PathTreeNode;

#[cfg(feature = "use_url")]
use url::Url;

pub type AsyncResourceReadResult = dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send;

// Resource flags
const RESOURCE_READONLY: u32 = 0x00000001;

#[derive(Copy, Clone, Debug)]
pub enum ResourceItemType {
    Directory(ResourceFsType),
    File(ResourceFsType),
}

#[derive(Copy, Clone, Debug)]
pub enum ResourceFsType {
    Native,
    Overlay(usize),
}

#[derive(Clone, Debug)]
pub struct ResourceItem {
    pub(crate) rtype: ResourceItemType,
    pub(crate) location: PathBuf,
    pub(crate) relative_path: Option<PathBuf>,
    pub(crate) filename_only: Option<OsString>,
    pub(crate) size: Option<u64>,
    #[allow(unused)]
    flags: u32,
}

impl ResourceItem {
    pub fn from_filename(filename: &str) -> Self {
        let mut new_path: PathBuf = PathBuf::from(".");
        new_path.push(filename.replace("/", "\\"));
        Self {
            rtype: ResourceItemType::File(ResourceFsType::Native),
            location: new_path.clone(),
            relative_path: None,
            filename_only: new_path.file_name().map(|s| s.to_os_string()),
            size: None,
            flags: 0,
        }
    }
}

pub struct ResourceManager {
    pub pm: PathManager,
    #[cfg(feature = "use_url")]
    pub base_url: Option<Url>,
    pub ignore_dirs: Vec<String>,
    pub overlays: Vec<ArchiveOverlay<std::io::Cursor<Vec<u8>>>>,
    #[cfg(target_arch = "wasm32")]
    manifest: ResourceManifest,
}

impl ResourceManager {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            pm: PathManager::new(base_path),
            #[cfg(feature = "use_url")]
            base_url: None,
            ignore_dirs: Vec::new(),
            overlays: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            manifest: ResourceManifest::default(),
        }
    }

    #[cfg(feature = "use_url")]
    pub fn set_base_url(&mut self, base_url: &Url) {
        self.base_url = Some(base_url.clone());
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn load_manifest(&mut self, manifest: ResourceLocation) -> Result<(), Error> {
        match manifest {
            ResourceLocation::Url(url) => {
                // Load the manifest from the URL
                let manifest_data = marty_web_helpers::fetch_url(&url)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to fetch manifest from URL '{}': {}", url, e))?;

                // Parse the manifest
                let manifest_str = String::from_utf8(manifest_data)?;

                // Deserialize the manifest FROM TOML
                let manifest_file: ManifestFile = toml::from_str(&manifest_str)?;

                self.manifest = ResourceManifest::new(&manifest_file.entries);
                self.manifest.debug();
            }
            ResourceLocation::FilePath(_) => {
                panic!("Don't use FilePath for wasm32 targets!");
            }
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn load_manifest(&mut self, _manifest: ResourceLocation) -> Result<(), Error> {
        log::debug!("load_manifest(): Not implemented for native build");
        Ok(())
    }

    pub fn resolve_path_from_filename(
        &mut self,
        resource: &str,
        file_name: impl AsRef<Path>,
    ) -> Result<PathBuf, Error> {
        let file_name = Self::validate_resource_filename(file_name.as_ref())?;
        let extension_filter = Path::new(file_name)
            .extension()
            .map(|extension| vec![extension.to_ascii_lowercase()]);
        let recursive = self
            .pm
            .resource_recurse(resource)
            .ok_or_else(|| anyhow::anyhow!("Resource path not found: {resource}"))?;
        let items = self.enumerate_items(resource, None, false, recursive, extension_filter)?;

        for item in items {
            if matches!(item.rtype, ResourceItemType::File(_))
                && item
                    .filename_only
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(file_name))
                && self.resource_file_is_contained(resource, &item)?
            {
                return Ok(item.location.clone());
            }
        }

        Err(anyhow::anyhow!(
            "Failed to resolve path for file: {}",
            file_name.to_string_lossy()
        ))
    }

    fn validate_resource_filename(file_name: &Path) -> Result<&OsStr, Error> {
        let file_name_text = file_name.as_os_str().to_string_lossy();
        if file_name_text.is_empty()
            || file_name_text == "."
            || file_name_text == ".."
            || file_name_text.contains(['/', '\\', ':'])
        {
            return Err(anyhow::anyhow!(
                "Resource filename must not contain a directory: {}",
                file_name.display()
            ));
        }

        let mut components = file_name.components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(file_name)), None) => Ok(file_name),
            _ => Err(anyhow::anyhow!(
                "Resource filename must be a single normal path component: {}",
                file_name.display()
            )),
        }
    }

    fn item_matches_extension(item: &ResourceItem, extension_filter: Option<&[OsString]>) -> bool {
        let Some(extension_filter) = extension_filter
        else {
            return true;
        };
        if extension_filter.is_empty() {
            return true;
        }

        item.location.extension().is_some_and(|extension| {
            extension_filter
                .iter()
                .any(|filter| extension.eq_ignore_ascii_case(filter))
        })
    }

    /// Check that a path is a valid item under a given root at the configured recursion level
    fn path_is_within_resource_scope(path: &Path, root: &Path, recursive: bool) -> bool {
        let Ok(relative_path) = path.strip_prefix(root)
        else {
            return false;
        };

        let mut component_count = 0;
        for component in relative_path.components() {
            if !matches!(component, Component::Normal(_)) {
                return false;
            }
            component_count += 1;
        }

        component_count > 0 && (recursive || component_count == 1)
    }

    pub fn from_config(base_path: PathBuf, config: &[PathConfigItem]) -> Result<Self, Error> {
        let mut rm = Self::new(base_path);
        for item in config {
            rm.pm.add_path(&item.resource, &item.path, item.create, item.recurse)?;
        }
        //rm.pm.create_paths()?;
        Ok(rm)
    }

    pub fn set_ignore_dirs(&mut self, dirs: Vec<String>) {
        self.ignore_dirs = dirs;
    }

    pub fn resource_path(&self, resource: &str) -> Option<PathBuf> {
        self.pm.resource_path(resource)
    }

    /// Return a unique filename for the given resource, base name, and extension.
    /// Names will be generated by appending digits to the base name until a unique name is found.
    pub fn get_available_filename(
        &mut self,
        resource: &str,
        base_name: &str,
        extension: Option<&str>,
    ) -> Result<PathBuf, Error> {
        let mut path = self
            .pm
            .resource_path(resource)
            .ok_or(anyhow::anyhow!("Resource path not found: {}", resource))?;

        // Generate a regex to extract a sequence of digits from a filename
        let re = Regex::new(r"(\d+)")?;
        let mut largest_num = 0;

        log::debug!("Finding unique filename in: {:?}", path);

        // First, generate a map of all items starting with 'base_name'
        let mut existing_basenames: HashSet<OsString> = HashSet::new();
        match self.enumerate_items(resource, None, false, false, None) {
            Ok(items) => {
                for item in items {
                    //log::debug!("Item: {:?}", item);
                    if let Some(filename) = item.filename_only.clone() {
                        if filename.to_string_lossy().to_ascii_lowercase().contains(base_name) {
                            //log::debug!("Found matching basename: {:?}", filename);

                            // Extract any number sequence from the filename
                            re.captures(
                                filename
                                    .to_str()
                                    .ok_or(anyhow::anyhow!("Failed to convert filename to string"))?,
                            )
                            .and_then(|caps| caps.get(1))
                            .and_then(|match_| match_.as_str().parse::<u32>().ok())
                            .map(|num| {
                                if num > largest_num {
                                    largest_num = num
                                }
                            });
                            existing_basenames.insert(filename.to_ascii_lowercase());
                        }
                    }
                }
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Failed to enumerate items in resource '{}': {}",
                    resource,
                    err
                ));
            }
        }

        // Generate unique names and check them against the map. We can start searching at
        // 'largest_num'
        let mut i = largest_num;
        let mut test_name_path = PathBuf::from(format!("{}{:04}", base_name, i));
        if let Some(ext) = extension {
            test_name_path.set_extension(ext);
        }
        let mut test_name = test_name_path.into_os_string();

        // for name in existing_basenames.iter() {
        //     log::debug!("Existing name: {} largest num: {}", name.to_str().unwrap(), largest_num);
        // }

        while existing_basenames.contains(&test_name) {
            i += 1;
            test_name_path = PathBuf::from(format!("{}{:04}", base_name, i));
            if let Some(ext) = extension {
                test_name_path.set_extension(ext);
            }
            test_name = test_name_path.into_os_string();
        }

        log::debug!("Found unique filename: {}", test_name.to_str().unwrap());

        path.push(test_name.clone());
        if let Some(ext) = extension {
            path.set_extension(ext);
        }

        // We should have a unique filename now. Check that the file exists before we return it
        // as one last sanity check.
        if ResourceManager::path_exists(&path) {
            log::error!(
                "Failed to create unique filename: File already exists: {}",
                path.to_str().unwrap()
            );
            return Err(anyhow::anyhow!(
                "Failed to create unique filename: File already exists: {}",
                path.to_str().unwrap()
            ));
        }
        Ok(path)
    }

    pub fn path_contains_dir(path: &Path, dir: &str) -> bool {
        path.iter().any(|component| component == dir)
    }

    pub fn path_contains_dirs(path: &Path, dirs: &Vec<&str>) -> bool {
        dirs.iter().any(|&dir| path.iter().any(|component| component == dir))
    }

    pub fn set_relative_paths_for_items(base: PathBuf, items: &mut [ResourceItem]) {
        // Strip the base path from all items.
        for item in items.iter_mut() {
            item.relative_path = Some(
                item.location
                    .strip_prefix(&base)
                    .unwrap_or(&item.location)
                    .to_path_buf(),
            );
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "martypc-resource-manager-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let temp_dir = std::env::temp_dir();
            let is_test_directory = self.path.starts_with(&temp_dir)
                && self
                    .path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("martypc-resource-manager-"));
            if is_test_directory {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    fn resource_manager(path: &Path, recurse: bool) -> ResourceManager {
        ResourceManager::from_config(
            PathBuf::new(),
            &[PathConfigItem {
                resource: "test".to_string(),
                path: path.to_string_lossy().into_owned(),
                create: false,
                recurse,
            }],
        )
        .unwrap()
    }

    #[test]
    fn resolver_handles_extensionless_and_mixed_case_filenames() {
        let test_dir = TestDirectory::new();
        fs::write(test_dir.path.join("README"), b"extensionless").unwrap();
        fs::write(test_dir.path.join("MiXeD.TxT"), b"mixed case").unwrap();
        let mut rm = resource_manager(&test_dir.path, false);

        assert_eq!(
            rm.resolve_path_from_filename("test", "readme")
                .unwrap()
                .canonicalize()
                .unwrap(),
            test_dir.path.join("README").canonicalize().unwrap()
        );
        assert_eq!(
            rm.resolve_path_from_filename("test", "MIXED.TXT")
                .unwrap()
                .canonicalize()
                .unwrap(),
            test_dir.path.join("MiXeD.TxT").canonicalize().unwrap()
        );

        let unfiltered = rm
            .enumerate_items("test", None, false, false, Some(Vec::new()))
            .unwrap();
        assert_eq!(unfiltered.len(), 2);
    }

    #[test]
    fn resolver_honors_configured_recursion_and_does_not_return_directories() {
        let test_dir = TestDirectory::new();
        let nested = test_dir.path.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("DEEP.BIN"), b"nested").unwrap();

        let mut non_recursive_rm = resource_manager(&test_dir.path, false);
        assert!(non_recursive_rm.resolve_path_from_filename("test", "DEEP.BIN").is_err());
        assert!(non_recursive_rm.resolve_path_from_filename("test", "nested").is_err());

        let mut recursive_rm = resource_manager(&test_dir.path, true);
        assert_eq!(
            recursive_rm
                .resolve_path_from_filename("test", "deep.bin")
                .unwrap()
                .canonicalize()
                .unwrap(),
            nested.join("DEEP.BIN").canonicalize().unwrap()
        );
    }

    #[test]
    fn resolver_rejects_directory_components_and_lexical_traversal() {
        let test_dir = TestDirectory::new();
        fs::write(test_dir.path.join("SAFE.BIN"), b"safe").unwrap();
        let mut rm = resource_manager(&test_dir.path, true);

        for filename in [
            "../SAFE.BIN",
            "..\\SAFE.BIN",
            "subdir/SAFE.BIN",
            "subdir\\SAFE.BIN",
            "C:\\SAFE.BIN",
            "SAFE.BIN:stream",
            "/SAFE.BIN",
            ".",
            "..",
        ] {
            assert!(
                rm.resolve_path_from_filename("test", filename).is_err(),
                "unexpectedly resolved {filename}"
            );
            assert!(
                rm.resolve_resource_path_for_write("test", filename).is_err(),
                "unexpectedly constructed a write path for {filename}"
            );
        }

        assert!(!ResourceManager::path_is_within_resource_scope(
            Path::new("root/../outside.bin"),
            Path::new("root"),
            true
        ));
    }

    #[test]
    fn write_paths_are_confined_to_the_resource_root() {
        let test_dir = TestDirectory::new();
        let rm = resource_manager(&test_dir.path, false);
        let path = rm.resolve_resource_path_for_write("test", "OUTPUT.BIN").unwrap();

        assert_eq!(path.parent().unwrap(), test_dir.path.canonicalize().unwrap());
    }

    #[test]
    fn resolver_and_writer_reject_symlinks_outside_the_resource_root() {
        let test_dir = TestDirectory::new();
        let resource_root = test_dir.path.join("resource");
        fs::create_dir(&resource_root).unwrap();
        let outside_file = test_dir.path.join("outside.bin");
        fs::write(&outside_file, b"outside").unwrap();
        let link = resource_root.join("LINK.BIN");

        #[cfg(unix)]
        let symlink_result = std::os::unix::fs::symlink(&outside_file, &link);
        #[cfg(windows)]
        let symlink_result = std::os::windows::fs::symlink_file(&outside_file, &link);
        if symlink_result.is_err() {
            // Creating symlinks can require an elevated token or Developer Mode on Windows.
            return;
        }

        let mut rm = resource_manager(&resource_root, false);
        assert!(rm.resolve_path_from_filename("test", "LINK.BIN").is_err());
        assert!(rm.resolve_resource_path_for_write("test", "LINK.BIN").is_err());
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside");
    }

    #[test]
    fn recursive_enumeration_does_not_follow_directory_symlinks_outside_the_resource_root() {
        let test_dir = TestDirectory::new();
        let resource_root = test_dir.path.join("resource");
        let outside_directory = test_dir.path.join("outside");
        fs::create_dir(&resource_root).unwrap();
        fs::create_dir(&outside_directory).unwrap();
        fs::write(outside_directory.join("SECRET.BIN"), b"secret").unwrap();
        let link = resource_root.join("outside-link");

        #[cfg(unix)]
        let symlink_result = std::os::unix::fs::symlink(&outside_directory, &link);
        #[cfg(windows)]
        let symlink_result = std::os::windows::fs::symlink_dir(&outside_directory, &link);
        if symlink_result.is_err() {
            return;
        }

        let mut rm = resource_manager(&resource_root, true);
        assert!(rm.resolve_path_from_filename("test", "SECRET.BIN").is_err());
    }
}
