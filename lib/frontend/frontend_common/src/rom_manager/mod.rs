/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

    frontend_common::rom_manager::mod.rs

    ROM management services for frontends.
*/

use crate::resource_manager::ResourceManager;
use anyhow::Error;
use marty_core::{
    machine::{MachineRomEntry, MachineRomManifest},
};
use serde::Deserialize;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    fmt::Display,
    path::{PathBuf},
};


#[derive(Debug)]
pub enum RomError {
    DirNotFound,
    RomNotFoundForMachine,
    RomNotFoundForRequirement(String),
    FileNotFound,
    FileError,
    Unimplemented,
    HashCollision,
}
impl std::error::Error for RomError {}
impl Display for RomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RomError::DirNotFound => write!(f, "ROM Directory was not found."),
            RomError::RomNotFoundForMachine => {
                write!(f, "A ROM was not found for the specified machine.")
            }
            RomError::RomNotFoundForRequirement(req) => {
                write!(f, "A ROM was not found for a specified feature requirement: {:?}.", req)
            }
            RomError::FileNotFound => write!(f, "File not found attempting to read ROM."),
            RomError::FileError => write!(f, "A File error occurred reading ROM."),
            RomError::Unimplemented => write!(f, "Functionality unimplemented."),
            RomError::HashCollision => write!(f, "Hash collision detected."),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub enum RomOrganization {
    #[default]
    Normal,
    Reversed,
    InterleavedEven,
    InterleavedOdd,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RomDefinitionFile {
    romset: Vec<RomSetDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RomDescriptor {
    md5: Option<String>,
    filename: Option<String>,
    addr: u32,
    size: Option<u32>,
    offset: Option<u32>,
    chip: Option<String>,
    org: Option<RomOrganization>,
    #[serde(skip)]
    present: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RomPatch {
    desc:    String,
    trigger: u32,
    addr:    u32,
    bytes:   Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RomCheckpoint {
    addr: u32,
    desc: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RomSetDefinition {
    alias: String,
    priority: u32,
    provides: Vec<String>,
    rom: Vec<RomDescriptor>,
    patch: Option<Vec<RomPatch>>,
    checkpoint: Option<Vec<RomCheckpoint>>,
}

pub struct RomSet {
    def: RomSetDefinition,
    roms: Vec<String>, // Key to rom hashmap
    complete: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RomFileCandidate {
    pub filename: String,
    pub path: PathBuf,
    pub md5: String,
    pub size: usize,
}

pub type NameMap = HashMap<String, (String, PathBuf)>; // Rom names resolve to md5sums

pub struct RomManager {
    rom_defs:    Vec<RomSetDefinition>,
    rom_def_map: HashMap<String, usize>,

    rom_sets_complete: Vec<String>,
    rom_set_active:    Option<String>,

    rom_sets_by_feature: HashMap<String, Vec<String>>,
    //rom_sets: HashMap<String, RomSet>, // Rom sets are hashed by 'alias'
    rom_candidates: HashMap<String, RomFileCandidate>,
    rom_candidate_name_map: NameMap,      // Rom names resolve to md5sums
    rom_paths: HashMap<String, PathBuf>,  // Rom paths are hashed by md5sum
    rom_images: HashMap<String, Vec<u8>>, // Rom images are hashed by md5sum
    features_available: Vec<String>,
    features_required: Vec<String>,
    rom_override: Option<String>, // Rom override forces a specific rom set alias to be loaded

    checkpoints_active: HashMap<u32, RomCheckpoint>,
    patches_active: HashMap<u32, RomPatch>,
}

impl Default for RomManager {
    fn default() -> Self {
        Self {
            rom_defs:    Vec::new(),
            rom_def_map: HashMap::new(),

            rom_sets_complete: Vec::new(),
            rom_set_active:    None,

            rom_sets_by_feature: HashMap::new(),
            //rom_sets: HashMap::new(), // Rom sets are hashed by 'alias'
            rom_candidates: HashMap::new(),
            rom_candidate_name_map: HashMap::new(),
            rom_paths: HashMap::new(),
            rom_images: HashMap::new(), // Rom images can be stored by name or md5 hash.
            features_available: Vec::new(),
            features_required: Vec::new(),
            rom_override: None, // Rom override forces a specific rom set alias to be loaded

            checkpoints_active: HashMap::new(),
            patches_active: HashMap::new(),
        }
    }
}

impl RomManager {
    pub fn new() -> Self {
        let slf = Self::default();

        slf
    }

    pub fn load_defs(&mut self, rm: &ResourceManager) -> Result<(), Error> {
        let mut rom_defs: Vec<RomSetDefinition> = Vec::new();

        // Get a file listing of the rom directory.
        let items = rm.enumerate_items("rom", true, true, None)?;

        // Filter out any non-toml files.
        let toml_defs: Vec<_> = items
            .iter()
            .filter_map(|item| {
                log::debug!("item: {:?}", item.full_path);
                if item.full_path.extension().is_some_and(|ext| ext == "toml") {
                    return Some(item);
                }
                None
            })
            .collect();

        log::debug!("load_defs(): Found {} rom definition files.", toml_defs.len());

        // Attempt to load each toml file as a rom definition file.
        for def in toml_defs {
            let mut loaded_def = self.load_def(&def.full_path)?;
            rom_defs.append(&mut loaded_def.romset);
        }

        // Create the map of rom set aliases to rom set indices.
        for (i, def) in rom_defs.iter().enumerate() {
            self.rom_def_map.insert(def.alias.clone(), i);
        }

        // We haven't had any errors yet, so we can assign the rom_defs as our final list.
        self.rom_defs = rom_defs;
        self.sort_by_feature();
        //self.print_rom_stats();
        Ok(())
    }

    fn load_def(&mut self, toml_path: &PathBuf) -> Result<RomDefinitionFile, Error> {
        let toml_str = std::fs::read_to_string(toml_path)?;
        let romdef = toml::from_str::<RomDefinitionFile>(&toml_str)?;

        log::debug!("Rom definition file loaded: {:?}", toml_path);
        Ok(romdef)
    }

    fn sort_by_feature(&mut self) {
        for def in self.rom_defs.iter() {
            for feature in def.provides.iter() {
                self.rom_sets_by_feature
                    .entry(feature.clone())
                    .and_modify(|e| {
                        e.push(def.alias.clone());
                    })
                    .or_insert(vec![def.alias.clone()]);
            }
        }

        // Now that all the rom sets have been added to the feature map, we need
        // to sort the feature map vectors by set priority.
        for (_feature, rom_set_vec) in self.rom_sets_by_feature.iter_mut() {
            rom_set_vec.sort_by(|a, b| {
                let a_set_idx = self.rom_def_map.get(a).unwrap();
                let b_set_idx = self.rom_def_map.get(b).unwrap();
                let a_set = &self.rom_defs[*a_set_idx];
                let b_set = &self.rom_defs[*b_set_idx];
                b_set.priority.cmp(&a_set.priority)
            });
        }
    }

    pub fn print_romset_stats(&mut self) {
        println!("Have {} ROM set definitions:", self.rom_defs.len());
        for def in self.rom_defs.iter() {
            println!(" {}", def.alias);
            for (i, rom) in def.rom.iter().enumerate() {
                println!(
                    "  rom {}: hash: {} file: {}",
                    i,
                    rom.md5.as_ref().unwrap_or(&String::from("")),
                    rom.filename.as_ref().unwrap_or(&String::from(""))
                );
            }
        }

        println!("Have {} complete ROM sets:", self.rom_sets_complete.len());
        for set_alias in self.rom_sets_complete.iter() {
            println!(" {}", set_alias);
        }

        println!("Complete sets support the following features:");
        for (feature, rom_set_vec) in self.rom_sets_by_feature.iter() {
            println!("  {}", feature);
            for (i, rom_set) in rom_set_vec.iter().enumerate() {
                if let Some(rom_set_idx) = self.rom_def_map.get(rom_set) {
                    println!(
                        "    {} ({}:{}), priority: {}",
                        rom_set, *rom_set_idx, i, self.rom_defs[*rom_set_idx].priority
                    );
                }
            }
        }
    }

    pub fn print_rom_stats(&mut self) {
        println!("Have {} ROM candidates:", self.rom_candidates.len());
        for (_key, rom_entry) in self.rom_candidates.iter() {
            println!("  Filename: {}", rom_entry.filename);
            println!("    Path: {}", rom_entry.path.to_str().unwrap_or_default());
            println!("    MD5:  {}", rom_entry.md5);
            println!("    Size: {}", rom_entry.size);
        }
    }

    /// Rescan the ROM specified by filename part for changes.
    /// Some ROMs may be expected to change (ie, during active ROM development) and when we reload
    /// the machine we need to reload the ROM, but the md5 may have changed. Calling this allows us to
    /// accomodate such ROM file changes.
    pub fn refresh_filename(&mut self, filename: String) -> Result<(), Error> {
        // Look up the old filename in the name map

        let mut rom_candidate;
        let rom_candidate_md5;

        if let Some((md5, _path)) = self.rom_candidate_name_map.remove(&filename) {
            // Take the old entry from the canddiate map.
            rom_candidate = self.rom_candidates.remove(&md5).unwrap();
            rom_candidate_md5 = md5;
        }
        else {
            return Err(anyhow::anyhow!("Rom {} not found in candidate name map.", filename));
        }

        log::debug!("refresh_filename(): Updating ROM hash for {:?}.", &rom_candidate.path);

        // Remove the old entry from the path map
        self.rom_paths.remove(&rom_candidate_md5);

        // Re-scan the file
        let file_vec = match std::fs::read(rom_candidate.path.clone()) {
            Ok(vec) => vec,
            Err(e) => {
                eprintln!("Error opening filename {:?}: {}", &rom_candidate.path, e);
                return Err(anyhow::anyhow!(
                    "Error opening filename {:?}: {}",
                    &rom_candidate.path,
                    e
                ));
            }
        };

        // Compute the md5 digest of the file and convert to string
        let file_digest = md5::compute(&file_vec);
        let file_digest_str = format!("{:x}", file_digest);
        rom_candidate.md5 = file_digest_str.clone();

        // Update the file size
        rom_candidate.size = file_vec.len();

        // Path and filename should not have changed.

        // stash clones of filename and path for the name map
        let map_filename = rom_candidate.filename.clone();
        let map_path = rom_candidate.path.clone();

        // Re-insert the candidate by md5
        self.rom_candidates.insert(file_digest_str.clone(), rom_candidate);

        // Update the candidate filename->md5 mapping.
        self.rom_candidate_name_map
            .entry(map_filename)
            .and_modify(|entry| {
                entry.0 = file_digest_str.clone();
            })
            .or_insert((file_digest_str.clone(), map_path));

        Ok(())
    }

    pub fn scan(&mut self, rm: &ResourceManager) -> Result<(), Error> {
        let roms = rm.enumerate_items("rom", true, true, None)?;

        // Clear the list of ROM candidates so we can rebuild it
        self.rom_candidates.clear();

        for rom_item in roms {
            let mut new_candidate: RomFileCandidate = Default::default();
            let file_vec = match std::fs::read(rom_item.full_path.clone()) {
                Ok(vec) => vec,
                Err(e) => {
                    eprintln!("Error opening filename {:?}: {}", &rom_item.full_path, e);
                    continue;
                }
            };

            // Compute the md5 digest of the file and convert to string
            let file_digest = md5::compute(&file_vec);
            let file_digest_str = format!("{:x}", file_digest);
            new_candidate.md5 = file_digest_str.clone();

            // Store the file size
            new_candidate.size = file_vec.len();

            // Store the path and filename
            new_candidate.path = rom_item.full_path.clone();
            new_candidate.filename = rom_item
                .filename_only
                .clone()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap_or_default();

            if new_candidate.filename.len() == 0 {
                eprintln!("Error: Non-UTF8 filename for {:?}", &rom_item.full_path);
                continue;
            }

            // stash clones of filename and path for the name map
            let map_filename = new_candidate.filename.clone();
            let map_path = new_candidate.path.clone();

            // Store the candidate by md5
            match self.rom_candidates.entry(file_digest_str.clone()) {
                Entry::Occupied(prev_entry) => {
                    eprintln!(
                        "Hash collision! Rom #1: {:?} Rom #2 {:?} both have hash {}. Rom #2 will be ignored.",
                        prev_entry.get().path,
                        new_candidate.path,
                        file_digest_str
                    );
                }
                Entry::Vacant(entry) => {
                    entry.insert(new_candidate);
                }
            }

            // Store the candidate by filename
            match self.rom_candidate_name_map.entry(map_filename) {
                Entry::Occupied(prev_entry) => {
                    eprintln!(
                        "Name collision! Rom #1: {:?} Rom #2 {:?} have the same name. Rom #2 will be ignored when referenced by name.",
                        prev_entry.get().1,
                        map_path,
                    );
                }
                Entry::Vacant(entry) => {
                    entry.insert((file_digest_str, map_path));
                }
            }
        }

        println!("ROM scan found {} unique ROMs.", self.rom_candidates.len());

        Ok(())
    }

    pub fn resolve_rom_sets(&mut self) -> Result<(), Error> {
        if self.rom_candidates.is_empty() {
            return Err(anyhow::anyhow!("No ROMs have been scanned."));
        }

        if self.rom_defs.is_empty() {
            return Err(anyhow::anyhow!("No ROM set definitions have been loaded."));
        }

        // Clear list of complete ROM sets.
        self.rom_sets_complete.clear();

        // Process the list of rom defs. We process by index to avoid borrowing issues
        // from using an iterator.
        for i in 0..self.rom_defs.len() {
            if let Ok(_) = self.resolve_rom_set(i) {
                self.rom_sets_complete.push(self.rom_defs[i].alias.clone());
            }
        }

        /*
        // Now we need to resolve rom sets by feature. For each complete rom set,
        // we enumerate the features it provides, and add it into the feature map.
        for rom_alias in self.rom_sets_complete.iter() {
            if let Some(rom_set_idx) = self.rom_def_map.get(rom_alias) {
                let rom_set = &self.rom_defs[*rom_set_idx];
                for feature in rom_set.provides.iter() {
                    log::warn!("Adding rom set {} to feature map for feature {}.", rom_alias, feature);
                    self.rom_sets_by_feature
                        .entry(feature.clone())
                        .and_modify(|e| {
                            e.push(rom_alias.clone());
                        })
                        .or_insert(vec![rom_alias.clone()]);
                }
            }
        }

         */

        Ok(())
    }

    pub fn resolve_rom_set(&mut self, set_idx: usize) -> Result<(), Error> {
        let set = &mut self.rom_defs[set_idx];

        // First, for any roms that are specified by filename, resolve the filename to a hash.
        for rom in set.rom.iter_mut() {
            // If the rom only has a filename, look it up in the candidate name map to get its discovered
            // hash, and then set the hash. That way we can assume all ROMs have a hash.
            if rom.md5.is_none() {
                if let Some(filename) = rom.filename.clone() {
                    if let Some((md5, _path)) = self.rom_candidate_name_map.get(&filename) {
                        rom.md5 = Some(md5.clone());
                        log::debug!("ROM filename: {} resolved to hash: {}.", filename, md5);
                    }
                    else {
                        return Err(anyhow::anyhow!(
                            "ROM name {} not found in candidate name map.",
                            filename
                        ));
                    }
                }
            }
        }

        // Create a set of all unique chips.
        let mut chip_set: HashSet<String> = HashSet::new();

        // ROMs specified in a set should all have a unique md5. Check for that now by adding the md5sums to a
        // HashSet and detecting collisions.
        let mut md5_set: HashSet<String> = HashSet::new();
        for rom in set.rom.iter_mut() {
            let md5 = rom.md5.clone().unwrap();
            if md5_set.contains(&md5) {
                return Err(anyhow::anyhow!(
                    "ROM set {} is invalid due to hash collision: {}.",
                    set.alias,
                    md5
                ));
            }
            else {
                md5_set.insert(md5);
            }

            // The 'chip' field provides a unique identifier for a ROM. If two ROM entries specify the
            // same 'chip' value, only one is required to be present. This allows for ROM dump variants,
            // etc. Again to normalize and simplify logic, if no 'chip' field is specified, we will set
            // it to the md5 hash of the ROM. Then we can assume that all ROMs have a 'chip' key.
            if rom.chip.is_none() {
                if let Some(md5) = rom.md5.clone() {
                    rom.chip = Some(md5.clone());
                }
            }

            // Check if this chip has already been resolved. If it has, we can skip this ROM.
            if chip_set.contains(&rom.chip.clone().unwrap()) {
                continue;
            }
            else {
                chip_set.insert(rom.chip.clone().unwrap());
            }

            // Now, we check that the ROM is present in the candidate list. If it is not, we mark it to
            // be dropped from the set.
            if let Some(md5) = rom.md5.clone() {
                if !self.rom_candidates.contains_key(&md5) {
                    log::debug!("ROM {} not found in candidate list. Dropping from set.", md5);
                    rom.present = false;
                }
                else {
                    rom.present = true;
                }
            }
        }

        // Drop any ROMs that are not present.
        set.rom.retain(|rom| rom.present);

        // Add ROMs to a HashMap of ROMs by chip, on first-come first-serve basis. The first ROM
        // that satisfies a chip will be used.
        let mut chip_map: HashMap<String, RomDescriptor> = HashMap::new();
        for rom in set.rom.iter() {
            let chip = rom.chip.clone().unwrap();
            chip_map.entry(chip).or_insert(rom.clone());
        }

        // Sanity check - we should have the same number of entries in chip_set as in chip_map.
        for chip in chip_set.iter() {
            if !chip_map.contains_key(chip) {
                return Err(anyhow::anyhow!(
                    "ROM set {} is invalid: ROM required to satisfy chip {} not found.",
                    set.alias,
                    chip
                ));
            }
        }

        Ok(())
    }

    /// Given a vector of ROM requirements, return a vector of ROM set names that satisfy the requirements.
    /// The logic here has the potential to be quite complex in certain situations, but the limited number
    /// of sets we support at the moment should permit a simple implementation.
    pub fn resolve_requirements(
        &mut self,
        required: Vec<String>,
        specified: Option<String>,
    ) -> Result<Vec<String>, Error> {
        let mut romset_vec = Vec::new();
        let mut provided_features = HashSet::new();

        if let Some(rom_vec) = self.rom_sets_by_feature.get(&String::from("ibm_basic")) {
            for rom in rom_vec.iter() {
                log::debug!("Found rom set for feature ibm_basic: {}", rom);
            }
        }
        else {
            log::debug!("No rom set found for feature ibm_basic.");
        }

        // If a specified rom is provided, we can add it first and mark its features as provided.
        if let Some(specified_rom) = specified {
            if let Some(rom_set_idx) = self.rom_def_map.get(&specified_rom) {
                let rom_set = &self.rom_defs[*rom_set_idx];
                for feature in rom_set.provides.iter() {
                    provided_features.insert(feature.clone());
                }
                romset_vec.push(specified_rom.clone());
            }
            else {
                return Err(anyhow::anyhow!(
                    "Specified rom set {} not found in rom set map.",
                    specified_rom
                ));
            }
        }

        for feature in required.iter() {
            log::debug!("Resolving feature: {}...", feature);

            if provided_features.contains(feature) {
                log::debug!("Feature {} already provided. Skipping.", feature);
                continue;
            }

            if let Some(feature_vec) = self.rom_sets_by_feature.get(feature) {
                if feature_vec.is_empty() {
                    return Err(anyhow::anyhow!(
                        "No ROM sets found for feature requirement: {}",
                        feature
                    ));
                }
                else {
                    // Get the list of provided features for the first rom set in the feature vector.

                    for rom in feature_vec.iter() {
                        log::debug!("Found rom set for feature {}: {}", feature, rom);
                        let rom_set_idx = self.rom_def_map.get(rom).unwrap();
                        let rom_set = &self.rom_defs[*rom_set_idx];

                        // Only add the rom set if NONE of its features are already provided.
                        let mut add_rom_set = true;
                        for feature in rom_set.provides.iter() {
                            if provided_features.contains(feature) {
                                log::debug!(
                                    "Rom set {} provides feature {} which is already provided. Skipping.",
                                    feature_vec[0],
                                    feature
                                );
                                add_rom_set = false;
                                continue;
                            }
                        }

                        if add_rom_set {
                            for feature in rom_set.provides.iter() {
                                provided_features.insert(feature.clone());
                            }
                            log::debug!("Adding ROM: {}", rom);
                            romset_vec.push(rom.clone());
                            break;
                        }
                    }
                }
            }
        }

        for required_feature in required.iter() {
            if !provided_features.contains(required_feature) {
                return Err(anyhow::anyhow!(
                    "No ROM sets found for feature requirement: {}",
                    required_feature
                ));
            }
        }

        Ok(romset_vec)
    }

    /// Create a MachineRomManifest struct given the list of ROM set names. This Manifest can be given to the
    /// emulator core to initialize a Machine.
    pub fn create_manifest(
        &mut self,
        rom_set_list: Vec<String>,
        rm: &ResourceManager,
    ) -> Result<MachineRomManifest, Error> {
        let mut new_manifest = MachineRomManifest::new();

        for rom_set in rom_set_list.iter() {
            // Retrieve the rom set definition for this rom set name
            let rom_set_idx = self
                .rom_def_map
                .get(rom_set)
                .ok_or(anyhow::anyhow!("Rom set {} not found in rom set map.", rom_set))?;

            let rom_set_def = &self.rom_defs[*rom_set_idx];

            // Iterate over the roms in the rom set definition, load them from disk and add them to the manifest.
            for rom_desc in rom_set_def.rom.iter() {
                let rom_md5 = rom_desc.md5.clone().unwrap();
                let rom_file = self.rom_candidates.get(&rom_md5).ok_or_else(|| {
                    anyhow::anyhow!("Rom {} not found in candidate list.", rom_desc.md5.as_ref().unwrap())
                })?;

                let mut rom_vec = rm.read_resource_from_path(&rom_file.path)?;

                // Handle rom organization
                // TODO: Interleaved organizations... double rom size and then interleave?
                match rom_desc.org {
                    None | Some(RomOrganization::Normal) => {
                        let mut offset_len = 0;
                        // Shorten ROM by dropping the first 'offset' bytes
                        if let Some(offset) = rom_desc.offset {
                            rom_vec = rom_vec[offset as usize..].to_vec();
                            offset_len = offset as usize;
                        }

                        // Truncate to 'size' if specified
                        if let Some(size) = rom_desc.size {
                            rom_vec.truncate(size as usize - offset_len);
                        }

                        new_manifest.roms.push(MachineRomEntry {
                            md5:  rom_desc.md5.clone().unwrap(),
                            addr: rom_desc.addr,
                            data: rom_vec,
                        })
                    }
                    Some(RomOrganization::Reversed) => {
                        rom_vec.reverse();

                        let mut offset_len = 0;
                        // Shorten ROM by dropping the first 'offset' bytes
                        if let Some(offset) = rom_desc.offset {
                            rom_vec = rom_vec[offset as usize..].to_vec();
                            offset_len = offset as usize;
                        }

                        // Truncate to 'size' if specified
                        if let Some(size) = rom_desc.size {
                            rom_vec.truncate(size as usize - offset_len);
                        }

                        new_manifest.roms.push(MachineRomEntry {
                            md5:  rom_desc.md5.clone().unwrap(),
                            addr: rom_desc.addr,
                            data: rom_vec,
                        })
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "ROM organization '{:?}' not implemented for ROM {}.",
                            rom_desc.org,
                            rom_desc.md5.as_ref().unwrap()
                        ))
                    }
                }
            }
        }

        Ok(new_manifest)
    }
}
