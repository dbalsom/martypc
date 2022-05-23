
use std::collections::HashMap;
use std::mem::discriminant;
use std::fs;
use std::path::PathBuf;

use lazy_static::lazy_static;

use crate::machine::{MachineType};

pub enum RomError {
    DirNotFound,
    RomNotFoundForMachine,
    FileNotFound,
    FileError
}

#[derive(Debug)]
pub enum RomType {
    BIOS,
    BASIC,
    Diagnostic,
}

pub struct RomPatch {
    desc: &'static str,
    address: usize,
    bytes: Vec<u8>
}


pub struct RomDescriptor {
    rom_type: RomType,
    filename: String, 
    machine_type: MachineType,
    priority: u32,
    address: usize,
    size: usize,
    has_basic: bool,
    can_use_basic: bool,
    
    patches: Vec<RomPatch>,
    checkpoints: HashMap<usize, &'static str>,
}

lazy_static! {
    static ref ROM_IMAGES: HashMap<&'static str, RomDescriptor> = {
        let mut m = HashMap::from([(
            "6a1ed4e3f500d785a01ff4d3e000d79c", 
            RomDescriptor {
                rom_type: RomType::BIOS,
                filename: String::new(),
                machine_type: MachineType::IBM_PC_5150,
                priority: 0,
                address: 0xFE000,
                size: 8192,
                has_basic: false,
                can_use_basic: true,
                patches: 
                    vec![
                    RomPatch{
                        desc: "Patch DMA check failure: JZ->JNP",
                        address: 0xFE130,
                        bytes: vec![0xEB, 0x03]
                    },
                    RomPatch{
                        desc: "Patch ROM checksum failure: JNZ-JZ",
                        address: 0xFE0D8,
                        bytes: vec![0x74, 0xD5]
                    }],   
                checkpoints:
                    HashMap::from([
                        (0xfe01a, "RAM Check Routine"),
                        (0xfe05b, "8088 Processor Test"),
                        (0xfe0b0, "ROS Checksum"),
                        (0xfe0da, "8237 DMA Initialization Test"),
                        (0xfe117, "DMA Controller test"),
                        (0xfe158, "Base 16K Read/Write Test"),
                        (0xfe235, "8249 Interrupt Controller Test"),
                        (0xfe285, "8253 Timer Checkout"),
                        (0xfe33b, "ROS Checksum II"),
                        (0xfe352, "Initialize CRTC Controller"),
                        (0xfe3af, "Video Line Test"),
                        (0xfe3c0, "CRT Interface Lines Test"),
                        (0xfe630, "Error Beep"),
                        (0xfe666, "Beep"),
                        (0xfe688, "Keyboard Reset"),
                        (0xfe6b2, "Blink LED Interrupt"),
                        (0xfe6ca, "Print Message"),
                        (0xfe6fa, "Bootstrap Loader"),
                        (0xf6000, "ROM BASIC"),
                    ])                                   
            }
        )]);

        m.insert(
            "6338a9808445de12109a2389b71ee2eb",
            RomDescriptor {
                rom_type: RomType::BIOS,
                filename: String::new(),
                machine_type: MachineType::IBM_PC_5150,
                priority: 2,
                address: 0xFE000,
                size: 8192,                
                has_basic: false,
                can_use_basic: true,                
                patches: Vec::new(),
                checkpoints: HashMap::new()                   
            }        
        );
        m.insert(
            "f453eb2df6daf21ec644d33663d85434",
            RomDescriptor {
                rom_type: RomType::BIOS,
                filename: String::new(),
                machine_type: MachineType::IBM_PC_5150,
                priority: 3,
                address: 0xFE000,
                size: 8192,                
                has_basic: false,
                can_use_basic: true,                
                patches: Vec::new(),
                checkpoints: HashMap::new()                  
            }        
        );        
        m
    };
}

pub struct RomManager {

    machine_type: MachineType,
    
    current_bios: String,
    current_bios_vec: Vec<u8>,
    current_basic: String,
    current_basic_vec: Vec<u8>,
    current_diag: String,

    have_basic: bool,
}

impl RomManager {

    pub fn new(machine_type: MachineType) -> Self {
        Self {
            machine_type,
            current_bios: String::new(),
            current_bios_vec: Vec::new(),
            current_basic: String::new(),
            current_basic_vec: Vec::new(),
            current_diag: String::new(),
            have_basic: false,
        }
    }

    pub fn try_load_from_dir(&mut self, path: &str) -> Result<bool, RomError> {
        
        let mut valid_bios: Vec<(String, PathBuf)> = Vec::new();
        let mut valid_basic: Vec<(String, PathBuf)> = Vec::new();
        let mut valid_diag: Vec<(String, PathBuf)> = Vec::new();
        
        let path_r = fs::read_dir(path);

        let dir = match path_r {
            Ok(dir) => dir,
            Err(_) => return Err(RomError::DirNotFound)
        };

        // See if any file is candidate 
        for entry in dir {
            if let Ok(entry) = entry {

                let file_vec = match std::fs::read(entry.path()) {
                    Ok(vec) => vec,
                    Err(e) => {
                        eprintln!("Error opening filename {:?}: {}", entry.path(), e);
                        return Err(RomError::FileNotFound);
                    }
                };                

                let file_digest = md5::compute(file_vec);
                let file_digest_str = format!("{:x}", file_digest);
            
                match RomManager::get_romdesc(file_digest_str.as_str()) {
                    Some(rom) => {
                        if discriminant(&rom.machine_type) == discriminant(&self.machine_type) {
                            // This ROM matches the machine we're looking for
                            match rom.rom_type {
                                RomType::BIOS => {
                                    valid_bios.push((file_digest_str.clone(), entry.path()));
                                }
                                RomType::BASIC => {
                                    valid_basic.push((file_digest_str.clone(), entry.path()));
                                }
                                RomType::Diagnostic => {
                                    valid_diag.push((file_digest_str.clone(), entry.path()));
                                }
                            }
                            println!("Found {:?} file for machine {:?}: {:?} MD5: {}", rom.rom_type, self.machine_type, entry.path(), file_digest_str);
                        }
                    },
                    None => {
                        continue;
                    }
                };
            }
        }

        // We now have vectors of ROM candidates, need some way to prefer one or the other
        // We use a 'priority' field in the RomDescriptor, higher priority ROM is preferred
        // Generally these are the newest versions of each ROM
        valid_bios.sort_by(|a,b| {
            let desc1 = RomManager::get_romdesc(a.0.as_str()).unwrap();
            let desc2 = RomManager::get_romdesc(b.0.as_str()).unwrap();
            desc2.priority.cmp(&desc1.priority)
        });

        if valid_bios.len() > 0 {
            println!("Using bios file: {:?} ", valid_bios[0].1);
        }
        else {
            return Err(RomError::RomNotFoundForMachine);
        }

        self.current_bios = valid_bios[0].0.clone();
        Ok(true)
    }

    pub fn get_romdesc(key: &str) -> Option<&RomDescriptor> {
        ROM_IMAGES.get(key)
    }

    pub fn has_basic(&self) -> bool {
        self.have_basic
    }

    pub fn get_checkpoint(&self, addr: usize) -> Option<String> {

        match ROM_IMAGES.get(&self.current_bios as &str) {
            Some(rom) => {
                match rom.checkpoints.get(&addr) {
                    Some(cp) => Some(cp.to_string()),
                    None => None
                }
            }
            None => None
        }
    }
}