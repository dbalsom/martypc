/*  
    rom_manager.rs
    ROM Manager module

    The ROM Manager is responsible for enumerating roms in the specified folder and making them 
    available to the emulator, based on machine type and requested features.

    Currently, ROMs to be loaded are just copied into the system's address space with a read-only 
    flag set.
    It could be argued a better design would be to make each ROM a memory-mapped device, although
    adding more entries to the memory map vector might slow down all read/writes.

    The ROM/ROM set definition structures should probably be moved to an external JSON file.

    The ROM manager supports the notion of "Feature ROMs" that are prerequisites for a 
    certain machine feature. For example, an EGA BIOS ROM should only be loaded if an EGA card
    is actually configured on the virtual machine, and conversely, if an EGA card is 
    specified the EGA BIOS ROM is required.
*/

#![allow(dead_code)] 

use std::collections::HashMap;
//use std::ffi::OsString;
use std::mem::discriminant;
use std::fs;
use std::path::PathBuf;
use std::cell::Cell;

use crate::config::MachineType;
use crate::bus::{BusInterface, MEM_CP_BIT};

pub const BIOS_READ_CYCLE_COST: u32 = 4;

pub enum RomError {
    DirNotFound,
    RomNotFoundForMachine,
    RomNotFoundForFeature(RomFeature),
    FileNotFound,
    FileError
}

pub enum RomInterleave {
    None,
    Odd,
    Even
}

pub enum RomOrder {
    Normal,
    Reversed
}

#[derive (Copy, Clone, Debug, PartialEq)]
pub enum RomFeature {
    XebecHDC,
    Basic,
    EGA,
    VGA
}

#[derive(Debug)]
pub enum RomType {
    BIOS,
    BASIC,
    Diagnostic,
}

#[derive (Clone)]
pub struct RomPatch {
    desc: &'static str,
    checkpoint: u32,
    address: u32,
    bytes: Vec<u8>,
    patched: bool
}

#[derive (Clone)]

pub struct RomSet {
    machine_type: MachineType,
    priority: u32,
    reset_vector: (u16, u16),
    roms: Vec<&'static str>,
    is_complete: Cell<bool>,
}

pub struct RomDescriptor {
    rom_type: RomType,
    present: bool,
    filename: PathBuf, 
    machine_type: MachineType,
    feature: Option<RomFeature>,
    order: RomOrder,
    interleave: RomInterleave,
    optional: bool,
    priority: u32,
    address: u32,
    offset: u32,
    size: usize,
    cycle_cost: u32,
    patches: Vec<RomPatch>,
    checkpoints: HashMap<u32, &'static str>,
}

pub struct RomManager {

    machine_type: MachineType,
    
    rom_sets: Vec<RomSet>,
    rom_sets_complete: Vec<RomSet>,
    rom_set_active: Option<RomSet>,
    checkpoints_active: HashMap<u32, &'static str>,
    patches_active: HashMap<u32, RomPatch>,
    rom_defs: HashMap<&'static str, RomDescriptor>,
    rom_images: HashMap<&'static str, Vec<u8>>,
    features_available: Vec<RomFeature>,
    features_requested: Vec<RomFeature>
}

impl RomManager {

    pub fn new(machine_type: MachineType, features_requested: Vec<RomFeature>) -> Self {
        Self {
            machine_type,

            rom_sets: Vec::from([
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 0,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "6338a9808445de12109a2389b71ee2eb",  // 5150 BIOS v1 04/24/81
                        "2ad31da203a49b504fad3a34af0c719f",  // Basic v1.0
                        "eb28f0e8d3f641f2b58a3677b3b998cc",  // Basic v1.01
                        //"66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 1,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "6a1ed4e3f500d785a01ff4d3e000d79c", // 5150 BIOS v2 10/19/81
                        "2ad31da203a49b504fad3a34af0c719f",  // Basic v1.0
                        "eb28f0e8d3f641f2b58a3677b3b998cc",  // Basic v1.01
                        //"66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 2,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "f453eb2df6daf21ec644d33663d85434", // 5150 BIOS v3 10/27/82
                        "2ad31da203a49b504fad3a34af0c719f",  // Basic v1.0
                        "eb28f0e8d3f641f2b58a3677b3b998cc",  // Basic v1.01
                        //"66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 10,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "3a0eacac07f1020b95ce06043982dfd1" // Supersoft Diagnostic ROM
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 10,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "b612305db2df43f88f9fb7f9b42d696e" // add.bin test suite
                    ]
                },    
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 11,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "7c075d48c950ef1d2900c1a10698ac6c" // bitwise.bin test suite
                    ]
                },      
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 12,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "a3e85d6807b8f92547681eaca5fbb92f" // bcdcnv.bin test suite
                    ]
                },  
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 13,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "6b0a52be2b82fbfaf0e00b0c195c11c1" // cmpneg.bin test suite
                    ]
                },    
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 14,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "87e6183b7a3f9e6f797e7bea092bc74d" // control.bin test suite
                    ]
                },                   
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 15,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "d0d91c22fce1d2d57fa591190362d0a8" // datatrnf.bin test suite
                    ]
                },                
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 16,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "19a32b41480d0e7a6f77f748eaa231c9" // div.bin test suite
                    ]
                },   
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 17,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "4cee4ef637299fe7e48196d3da1eb846" // interrupt.bin test suite
                    ]
                },       
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 18,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "edcd652c64df0bfb923d5499ea713992" // jmpmov.bin test suite
                    ]
                },      
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 19,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "bdd8489b68773ccaeab434e985409ba6" // jump1.bin test suite
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 20,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "c9243ef5e2c6b6723db313473bf2519b" // jump2.bin test suite
                    ]
                },  
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 21,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "7e81ea262fec23f0c20c8e11e7b2689a" // mul.bin test suite
                    ]
                }, 
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 22,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "cb8c54acd992166a67ea3927131cf219" // rep.bin test suite
                    ]
                },       
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 23,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "317e7c9ce01851b6227ac01d48c7778e" // rotate.bin test suite
                    ]
                },  
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 24,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "b2e5c51c10a1ce987cccebca8d0ba5c2" // segpr.bin test suite
                    ]
                },         
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 25,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "3aa4d3110127adfa652812f0428d620a" // shifts.bin test suite
                    ]
                },        
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 26,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "845902b2b98e43580c3b44a3c09c8376" // strings.bin test suite
                    ]
                }, 
                RomSet {
                    machine_type: MachineType::IBM_PC_5150,
                    priority: 27,
                    is_complete: Cell::new(false),
                    reset_vector: (0xF000, 0),
                    roms: vec![
                        "2e8df7c7c23646760dd18749d03b7b5a" // sub.bin test suite
                    ]
                },  
                RomSet {
                    machine_type: MachineType::IBM_XT_5160,
                    priority: 3,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "1a2ac1ae0fe0f7783197e78da8b3126c", // 5160 BIOS u18 v11/08/82
                        "e816a89768a1bf4b8d52b454d5c9d1e1", // 5160 BIOS u19 v11/08/82
                        "66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                        "0636f46316f3e15cb287ce3da6ba43a1", // IBM EGA card
                        "2057a38cb472300205132fb9c01d9d85", // IBM VGA card
                        "2c8a4e1db93d2cbe148b66122747e4f2", // IBM VGA card trimmed
                        "5455948e02dcb8824af45f30e8e46ce6", // SeaBios VGA BIOS
                    ]
                },                                                                                                                
                RomSet {
                    machine_type: MachineType::IBM_XT_5160,
                    priority: 4,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "fd9ff9cbe0a8f154746ccb0a33f6d3e7", // 5160 BIOS u18 v01/10/86
                        "f051b4bbc3b60c3a14df94a0e4ee720f", // 5160 BIOS u19 v01/10/86
                        "66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                        "0636f46316f3e15cb287ce3da6ba43a1", // IBM EGA card
                        "2057a38cb472300205132fb9c01d9d85", // IBM VGA card
                        "2c8a4e1db93d2cbe148b66122747e4f2", // IBM VGA card trimmed
                        "5455948e02dcb8824af45f30e8e46ce6", // SeaBios VGA BIOS
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_XT_5160,
                    priority: 5,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "9696472098999c02217bf922786c1f4a", // 5160 BIOS u18 v05/09/86
                        "df9f29de490d7f269a6405df1fed69b7", // 5160 BIOS u19 v05/09/86
                        "66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                        "0636f46316f3e15cb287ce3da6ba43a1", // IBM EGA card
                        "2057a38cb472300205132fb9c01d9d85", // IBM VGA card
                        "2c8a4e1db93d2cbe148b66122747e4f2", // IBM VGA card trimmed            
                        "5455948e02dcb8824af45f30e8e46ce6", // SeaBios VGA BIOS        
                    ]
                },
                RomSet {
                    machine_type: MachineType::IBM_XT_5160,
                    priority: 5,
                    is_complete: Cell::new(false),
                    reset_vector: (0xFFFF, 0),
                    roms: vec![
                        "9696472098999c02217bf922786c1f4a", // 5160 BIOS u18 v05/09/86
                        "df9f29de490d7f269a6405df1fed69b7", // 5160 BIOS u19 v05/09/86
                        "66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                        "0636f46316f3e15cb287ce3da6ba43a1", // IBM EGA card
                        "2057a38cb472300205132fb9c01d9d85", // IBM VGA card
                        "2c8a4e1db93d2cbe148b66122747e4f2", // IBM VGA card trimmed            
                        "5455948e02dcb8824af45f30e8e46ce6", // SeaBios VGA BIOS        
                    ]
                }

            ]),
            rom_sets_complete: Vec::new(),
            rom_set_active: None,
            checkpoints_active: HashMap::new(),
            patches_active: HashMap::new(),
            rom_defs: HashMap::from([(
                "6338a9808445de12109a2389b71ee2eb", // 5150 BIOS v1 04/24/81
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 0,
                    address: 0xFE000,
                    offset: 0,
                    size: 8192,
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(), 
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
                            (0xfe3f8, "Additional R/W Storage Test"),
                            (0xfe630, "Error Beep"),
                            (0xfe666, "Beep"),
                            (0xfe688, "Keyboard Reset"),
                            (0xfe6b2, "Blink LED Interrupt"),
                            (0xfe6ca, "Print Message"),
                            (0xfe6f2, "Bootstrap Loader"),
                            (0xFEF33, "FDC Wait for Interrupt"),
                            (0xFEF47, "FDC Interrupt Timeout"),
                            (0xf6000, "ROM BASIC"),
                        ])                                   
                }
            ),(
                "6a1ed4e3f500d785a01ff4d3e000d79c", // 5150 BIOS v2 10/19/81
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,     
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 2,
                    address: 0xFE000,
                    offset: 0,
                    size: 8192,       
                    cycle_cost: BIOS_READ_CYCLE_COST,                         
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }        
            ),(
                "f453eb2df6daf21ec644d33663d85434",
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,         
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 3,
                    address: 0xFE000,
                    offset: 0,
                    size: 8192,       
                    cycle_cost: BIOS_READ_CYCLE_COST,                               
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                  
                }      
            ),(
                "2ad31da203a49b504fad3a34af0c719f",
                RomDescriptor {
                    rom_type: RomType::BASIC,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,       
                    interleave: RomInterleave::None,
                    optional: true,
                    priority: 1,
                    address: 0xF6000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "eb28f0e8d3f641f2b58a3677b3b998cc",
                RomDescriptor {
                    rom_type: RomType::BASIC,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,    
                    interleave: RomInterleave::None,
                    optional: true,
                    priority: 2,
                    address: 0xF6000,    
                    offset: 0,   
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    size: 32768,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "1a2ac1ae0fe0f7783197e78da8b3126c", // BIOS_5160_08NOV82_U18_1501512.BIN
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,    
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 1,
                    address: 0xF8000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "e816a89768a1bf4b8d52b454d5c9d1e1", // BIOS_5160_08NOV82_U19_5000027_27256.BIN
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,   
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 1,
                    address: 0xF0000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: vec![
                        RomPatch {
                            desc: "Patch ROS checksum routine",
                            checkpoint: 0xFE0AE,
                            address: 0xFE0D7,
                            bytes: vec![0xEB, 0x00],
                            patched: false
                        },
                        RomPatch{
                            desc: "Patch RAM Check Routine for faster boot",
                            checkpoint: 0xFE46A,
                            address: 0xFE49D,
                            bytes: vec![0x90, 0x90, 0x90, 0x90, 0x90],
                            patched: false
                        },                        
                    ],
                    checkpoints: HashMap::from([
                        (0xFE01A, "RAM Check Routine"),
                        (0xFE05B, "8088 Processor Test"),
                        (0xFE0AE, "ROS Checksum Test I"),
                        (0xFE0D9, "8237 DMA Initialization Test"),
                        (0xFE135, "Start DRAM Refresh"),
                        (0xFE166, "Base 16K RAM Test"),
                        (0xFE242, "Initialize CRTC Controller"),
                        (0xFE329, "8259 Interrupt Controller Test"),
                        (0xFE35D, "8253 Timer Checkout"),
                        (0xFE3A2, "Keyboard Test"),
                        (0xFE3DE, "Setup Interrupt Vector Table"),
                        (0xFE418, "Expansion I/O Box Test"),
                        (0xFE46A, "Additional R/W Storage Test"),
                        /*
                        (0xFE53C, "Optional ROM Scan"),
                        (0xFE55B, "Diskette Attachment Test"),
                        */
                    ])
                }
            ),(
                "fd9ff9cbe0a8f154746ccb0a33f6d3e7", // BIOS_5160_10JAN86_U18_62X0851_27256_F800.BIN
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,    
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 1,
                    address: 0xF8000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "f051b4bbc3b60c3a14df94a0e4ee720f", // BIOS_5160_10JAN86_U19_62X0854_27256_F000.BIN
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,   
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 1,
                    address: 0xF0000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "9696472098999c02217bf922786c1f4a", // BIOS_5160_09MAY86_U18_59X7268_62X0890_27256_F800.BIN 
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,       
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 1,
                    address: 0xF8000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()
                }
            ),(
                "df9f29de490d7f269a6405df1fed69b7",  // BIOS_5160_09MAY86_U19_62X0819_68X4370_27256_F000.BIN
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: None,
                    order: RomOrder::Normal,      
                    interleave: RomInterleave::None,             
                    optional: false,
                    priority: 1,
                    address: 0xF0000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: vec![
                        RomPatch {
                            desc: "Patch ROS checksum routine",
                            checkpoint: 0xFE0AC,
                            address: 0xFE0D5,
                            bytes: vec![0xEB, 0x00],
                            patched: false
                        },
                        RomPatch{
                            desc: "Patch RAM Check Routine for faster boot",
                            checkpoint: 0xFE499,
                            address: 0xFE4EA,
                            bytes: vec![0x90, 0x90, 0x90, 0x90, 0x90],
                            patched: false
                        },
                        /*
                        RomPatch{
                            desc: "Patch out PIC IMR register test",
                            checkpoint: 0xFE000,
                            address: 0xFE36A,
                            bytes: vec![0x90, 0x90],
                            patched: false
                        }
                        */
                    ],  
                    checkpoints: HashMap::from([
                        (0xfe01a, "RAM Check Routine"),
                        (0xfe05b, "8088 Processor Test"),
                        (0xFE0AC, "ROS Checksum Test I"),
                        (0xFE0D7, "8237 DMA Initialization Test"),
                        (0xFE136, "Start DRAM Refresh"),
                        (0xFE166, "Base 16K RAM Test"),
                        (0xFE1DA, "Initialize 8259 PIC"),
                        (0xFE20B, "Determine Configuration and Mfg Mode"),
                        //(0xFECA0, "Wait Routine"),
                        (0xFE261, "Initialize CRTC Controller"),
                        (0xFE2EE, "Video Line Test"),
                        (0xFE35C, "8259 Interrupt Controller Test"),
                        (0xFE38F, "8253 Timer Checkout"),
                        (0xFE3D4, "Keyboard Test"),
                        (0xFE40F, "Setup Interrupt Vector Table"),
                        (0xFE448, "Expansion I/O Box Test"),
                        (0xFE499, "Additional R/W Storage Test"),
                        (0xFE53C, "Optional ROM Scan"),
                        (0xFE55B, "Diskette Attachment Test"),

                    ]) 
                }
            ),(
                "66631d1a095d8d0d54cc917fbdece684", // IBM / Xebec 20 MB Fixed Disk Drive Adapter
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::XebecHDC),
                    order: RomOrder::Normal,      
                    interleave: RomInterleave::None,              
                    optional: false,
                    priority: 1,
                    address: 0xC8000,
                    offset: 0,
                    size: 4096,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::from([
                        (0xC8003, "HDC Expansion Init"),   
                        (0xC8117, "HDC Disk Reset"),
                        (0xC8596, "HDC Status Timeout"),
                        (0xC8192, "HDC Bootstrap Loader"),
                        (0xC81FF, "HDC Boot From Fixed Disk")
                    ])          
                }
            ),
            (
                // IBM EGA Card ROM (86box Normal Order)
                // ibm_6277356_ega_card_u44_27128.bin
                "528455ed0b701722c166c6536ba4ff46",
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::EGA),
                    order: RomOrder::Normal,  
                    interleave: RomInterleave::None,                  
                    optional: true,
                    priority: 1,
                    address: 0xC0000,
                    offset: 0,
                    size: 16384,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::from([
                        (0xC0003, "EGA Expansion Init"),
                        (0xC009B, "EGA DIP Switch Sense"),
                        (0xC0205, "EGA CD Presence Test"),
                        (0xC037C, "EGA VBlank Bit Test"),
                        (0xC0D20, "EGA Error Beep"),
                        (0xC03F6, "EGA Diagnostic Dot Test"),
                        (0xC0480, "EGA Mem Test"),
                        (0xC068F, "EGA How Big Test")
                    ])         
                }
            ),            
            (
                // IBM EGA Card ROM (Reversed)
                // ibm_6277356_ega_card_u44_27128.bin
                "0636f46316f3e15cb287ce3da6ba43a1",
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::EGA),
                    order: RomOrder::Reversed,  
                    interleave: RomInterleave::None,                  
                    optional: true,
                    priority: 1,
                    address: 0xC0000,
                    offset: 0,
                    size: 16384,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::from([
                        (0xC0003, "EGA Expansion Init"),
                        (0xC009B, "EGA DIP Switch Sense"),
                        (0xC0205, "EGA CD Presence Test"),
                        (0xC037C, "EGA VBlank Bit Test"),
                        (0xC0D20, "EGA Error Beep"),
                        (0xC03F6, "EGA Diagnostic Dot Test"),
                        (0xC0480, "EGA Mem Test"),
                        (0xC068F, "EGA How Big Test")
                    ])         
                }
            ),
            (
                "2057a38cb472300205132fb9c01d9d85", // IBM VGA Card ROM (32K)
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::VGA),
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,          
                    optional: true,
                    priority: 1,
                    address: 0xC0000,
                    offset: 0x2000,                 // First 8k is unused
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    /*
                    patches: vec![
                        RomPatch {
                            desc: "Patch VGA vblank test",
                            checkpoint: 0xC2003,
                            address: 0xC224E,
                            bytes: vec![
                                0x90, 0x90, 0x90, 0x90, 
                                0x90, 0x90, 0x90, 0x90, 
                                0x90, 0x90, 0x90, 0x90, 
                                0x90, 0x90, 0x90, 0x90, 
                                0x90, 0x90]
                        }
                    ],
                    */
                    checkpoints: HashMap::from([
                        (0xC203B, "VGA Expansion Init"),
                        (0xC21F9, "VGA Vblank Test")
                    ])         
                }
            ),    
            (
                "2c8a4e1db93d2cbe148b66122747e4f2", // IBM VGA Card ROM (24K)
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::VGA),
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,          
                    optional: true,
                    priority: 1,
                    address: 0xC0000,
                    offset: 0,
                    size: 24576,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::from([
                    ])         
                }
            ), 
            (
                "5455948e02dcb8824af45f30e8e46ce6", // SeaBios VGA BIOS
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_XT_5160,
                    feature: Some(RomFeature::VGA),
                    order: RomOrder::Normal,  
                    interleave: RomInterleave::None,                 
                    optional: true,
                    priority: 1,
                    address: 0xC0000,
                    offset: 0,
                    size: 24576,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::from([
                    ])         
                }
            ), 
            (
                "3a0eacac07f1020b95ce06043982dfd1", // Supersoft PC/XT Diagnostic ROM
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,        
                    optional: false,
                    priority: 10,
                    address: 0xFE000,
                    offset: 0,
                    size: 32768,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()        
                }
            ),(
                "b612305db2df43f88f9fb7f9b42d696e", // add.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,   
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "7c075d48c950ef1d2900c1a10698ac6c", // bitwise.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,  
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "a3e85d6807b8f92547681eaca5fbb92f", // bcdcnv.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "6b0a52be2b82fbfaf0e00b0c195c11c1", // cmpneg.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,     
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "d0d91c22fce1d2d57fa591190362d0a8", // datatrnf.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "87e6183b7a3f9e6f797e7bea092bc74d", // control.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "19a32b41480d0e7a6f77f748eaa231c9", // div.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None, 
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "4cee4ef637299fe7e48196d3da1eb846", // interrupt.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "edcd652c64df0bfb923d5499ea713992", // jmpmov.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "bdd8489b68773ccaeab434e985409ba6", // jump1.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "c9243ef5e2c6b6723db313473bf2519b", // jump2.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "7e81ea262fec23f0c20c8e11e7b2689a", // mul.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "cb8c54acd992166a67ea3927131cf219", // rep.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "317e7c9ce01851b6227ac01d48c7778e", // rotate.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "b2e5c51c10a1ce987cccebca8d0ba5c2", // segpr.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "3aa4d3110127adfa652812f0428d620a", // shifts.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "845902b2b98e43580c3b44a3c09c8376", // strings.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            ),(
                "2e8df7c7c23646760dd18749d03b7b5a", // sub.bin test suite
                RomDescriptor {
                    rom_type: RomType::BIOS,
                    present: false,
                    filename: PathBuf::new(),
                    machine_type: MachineType::IBM_PC_5150,
                    feature: None,
                    order: RomOrder::Normal,
                    interleave: RomInterleave::None,
                    optional: false,
                    priority: 10,
                    address: 0xF0000,
                    offset: 0,
                    size: 65536,       
                    cycle_cost: BIOS_READ_CYCLE_COST,
                    patches: Vec::new(),
                    checkpoints: HashMap::new()                   
                }
            )                           
            
            
            ]),
            rom_images: HashMap::new(),
            features_available: Vec::new(),
            features_requested
        }
    }

    pub fn try_load_from_dir(&mut self, path: &str) -> Result<bool, RomError> {

        // Red in directory entries within the provided path
        let dir = match fs::read_dir(path) {
            Ok(dir) => dir,
            Err(_) => return Err(RomError::DirNotFound)
        };

        // Iterate through directory entries and check if we find any 
        // files that match rom definitions
        for entry in dir {
            if let Ok(entry) = entry {

                let file_vec = match std::fs::read(entry.path()) {
                    Ok(vec) => vec,
                    Err(e) => {
                        eprintln!("Error opening filename {:?}: {}", entry.path(), e);
                        continue;
                    }
                };

                // Compute the md5 digest of the file and convert to string
                let file_digest = md5::compute(file_vec);
                let file_digest_str = format!("{:x}", file_digest);
            
                let machine_type = self.machine_type;

                // Look up the md5 digest in our list of known rom files
                if let Some(rom) = self.get_romdesc_mut(file_digest_str.as_str()) {
                    if discriminant(&rom.machine_type) == discriminant(&machine_type) {
                        // This ROM matches the machine we're looking for, so mark it present
                        // and save its filename
                        rom.present = true;
                        rom.filename = entry.path();
                        log::debug!("Found {:?} file for machine {:?}: {:?} MD5: {}", rom.rom_type, machine_type, entry.path(), file_digest_str);
                    }
                }
            }
        }

        // Loop through all ROM set definitions for this machine type and mark which are complete
        // and them to a vec of complete rom sets
        for set in self.rom_sets.iter().filter(
            |r| discriminant(&self.machine_type) == discriminant(&r.machine_type)) {
                
                let mut required_rom_missing = false;
                for rom in &set.roms {

                    match self.get_romdesc(*rom) {
                        Some(romdesc) => {
                            
                            if !romdesc.optional && !romdesc.present {
                                // Required rom not found
                                required_rom_missing = true;
                            }
                        }
                        None => {
                            panic!("Invalid rom reference")
                        }
                    }
                }
                if !required_rom_missing {
                    set.is_complete.set(true);
                    self.rom_sets_complete.push(set.clone());
                }
            }

        // Sort the list of complete rom sets by priority
        self.rom_sets_complete.sort_by(|a,b| {
            let set1 = a.priority;
            let set2 = b.priority;
            set2.cmp(&set1)
        });

        for set in &self.rom_sets_complete {
            log::debug!("Found complete rom set, priority {}", set.priority)
        }

        if self.rom_sets_complete.len() == 0 {
            eprintln!("Couldn't find complete ROM set!");
            return Err(RomError::RomNotFoundForMachine);
        }

        // Select the active rom set from the highest priority complete set
        let mut rom_set_active = self.rom_sets_complete[0].clone();

        // Filter roms that are optional and missing
        rom_set_active.roms.retain(|rom| {
            let rom_desc = self.get_romdesc(rom).unwrap();
            rom_desc.present
        });

        // Filter roms that provide features that were not requested
        rom_set_active.roms.retain(|rom| {
            let rom_desc = self.get_romdesc(rom).unwrap();

            if let Some(feature) = rom_desc.feature {
                if !self.features_requested.contains(&feature) {
                    return false
                }
                else {
                    return true
                }
            }
            true
        });        

        // Now remove all but highest priority Basic images
        
        // Find highest priority Basic:
        let mut highest_priority_basic = 0;
        for rom in &rom_set_active.roms {
            let rom_desc = self.get_romdesc(rom).unwrap();
            if let RomType::BASIC = rom_desc.rom_type {
                if rom_desc.priority > highest_priority_basic {
                    highest_priority_basic = rom_desc.priority;
                }
            }
        }

        log::debug!("Highest priority BASIC: {}", highest_priority_basic);
        // Remove all lower priority Basics:
        rom_set_active.roms.retain(|rom| {
            let rom_desc = self.get_romdesc(rom).unwrap();
            match rom_desc.rom_type {
                RomType::BASIC => rom_desc.priority == highest_priority_basic,
                _=> true
            }
        });    

        // Load ROM images from active rom set
        for rom_str in &rom_set_active.roms {

            let rom_desc = self.get_romdesc(*rom_str).unwrap();
            let mut file_vec = match std::fs::read(&rom_desc.filename) {
                Ok(vec) => vec,
                Err(e) => {
                    eprintln!("Error opening filename {:?}: {}", rom_desc.filename, e);
                    return Err(RomError::FileNotFound);
                }               
            };

            // Reverse the rom if required
            if let RomOrder::Reversed = rom_desc.order {
                file_vec = file_vec.into_iter().rev().collect();
            }

            self.rom_images.insert(*rom_str, file_vec);
        }

        // Load checkpoints from active rom set
        for rom_str in &rom_set_active.roms {

            let rom_desc = self.get_romdesc(*rom_str).unwrap();

            let mut cp_map: HashMap<u32, &'static str> = HashMap::new();

            // Copy checkpoints for each rom in checkpoints_active for faster lookup
            // Since this will be looked up per-instruction
            for kv in rom_desc.checkpoints.iter() {
                cp_map.insert(*kv.0, kv.1);
            }

            self.checkpoints_active.extend(cp_map.iter());
        }

        log::debug!("Loaded {} checkpoints for active ROM set.", self.checkpoints_active.len());

        // Load patches from active rom set
        for rom_str in &rom_set_active.roms {
            let rom_desc = self.get_romdesc(*rom_str).unwrap();

            let mut patch_map: HashMap<u32, RomPatch> = HashMap::new();

            // Copy patches for each rom in patches_active for faster lookup
            // Since this will be looked up per-instruction
            for patch in rom_desc.patches.iter() {
                patch_map.insert(patch.checkpoint, (*patch).clone());
            }

            // Copy patches for each rom into patches_active for faster lookup
            // Since this will be looked up per-instruction
            self.patches_active.extend(patch_map);
        }

        // Load features from active rom set 
        for rom_str in &rom_set_active.roms {
            let rom_desc = self.get_romdesc(*rom_str).unwrap();

            match rom_desc.feature {
                Some(RomFeature::XebecHDC) => {
                    self.features_available.push(RomFeature::XebecHDC);
                },
                Some(RomFeature::EGA) => {
                    self.features_available.push(RomFeature::EGA);
                },
                Some(RomFeature::VGA) => {
                    self.features_available.push(RomFeature::VGA);
                },                
                _ => {}
            }
        }

        // Check that all requested features are avaialble
        for feature in &self.features_requested {
            if !self.features_available.contains(feature) {
                return Err(RomError::RomNotFoundForFeature(*feature));
            }
        }

        // Store active rom set 
        self.rom_set_active = Some(rom_set_active);

        println!("Loaded {} roms in romset.", self.rom_images.len());
        Ok(true)
    }

    pub fn get_romdesc(&self, key: &str) -> Option<&RomDescriptor> {
        self.rom_defs.get(key)
    }

    pub fn get_romdesc_mut(&mut self, key: &str) -> Option<&mut RomDescriptor> {
        self.rom_defs.get_mut(key)
    }

    /// Copy each from the active ROM set into memory.
    /// Only copy Feature ROMs if they match the list of requested features.
    pub fn copy_into_memory(&self, bus: &mut BusInterface) -> bool {

        if self.rom_sets_complete.is_empty() {
            return false;
        }

        for rom_str in &self.rom_set_active.as_ref().unwrap().roms {

            let rom_desc = self.get_romdesc(rom_str).unwrap();

            let load_rom = match rom_desc.feature {
                None => {
                    // ROMs not associated with a specific feature are always loaded
                    true
                }
                Some(feature) => self.features_requested.contains(&feature)
            };

            let rom_image_vec = self.rom_images.get(*rom_str).unwrap();

            if load_rom {
                match bus.copy_from(
                    &rom_image_vec[(rom_desc.offset as usize)..], 
                    rom_desc.address as usize, 
                    rom_desc.cycle_cost, 
                    true) {

                    Ok(_) => {
                        log::debug!("Mounted rom {:?} at location {:06X}", 
                            rom_desc.filename.as_os_str(),
                            rom_desc.address);
                    }
                    Err(e) => {
                        log::debug!("Failed to mount rom {:?} at location {:06X}: {}", 
                            rom_desc.filename.as_os_str(),
                            rom_desc.address,
                            e);
                    }
                }
            }
        }

        true
    }

    /// Sets the checkpoint bus flag for loaded checkpoints. We only try to look up the checkpoint
    /// for an address if this flag is set, for speed.
    pub fn install_checkpoints(&self,  bus: &mut BusInterface) {

        self.checkpoints_active.keys().for_each(|addr| {
            bus.set_flags(*addr as usize, MEM_CP_BIT);
        });

        self.patches_active.keys().for_each(|addr| {
            bus.set_flags(*addr as usize, MEM_CP_BIT);
        });        
    }

    /// Install the patch at the specified patching checkpoint. Mark the patch as installed.
    pub fn install_patch(&mut self, bus: &mut BusInterface, cp_address: u32) {

        let mut rom_str_vec = Vec::new();
        if let Some(rom_set) = &self.rom_set_active {
            for rom_str in &rom_set.roms {
                rom_str_vec.push(rom_str.clone());
            }
        }

        for rom_str in &rom_str_vec {
            if let Some(rom_desc) = self.get_romdesc_mut(rom_str) {
                //log::debug!("Found {} patches for ROM {}", rom_desc.patches.len(), rom_str );
                for patch in &mut rom_desc.patches {
                    
                    if !patch.patched && patch.checkpoint == cp_address {
                        match bus.patch_from(&patch.bytes, patch.address as usize) {
                            Ok(_) => {
                                log::debug!("Installed patch '{}' at address {:06X}", patch.desc, patch.address);
                            },
                            Err(e) => {
                                log::debug!("Error installing patch '{}' at address {:06X}; {}", patch.desc, patch.address, e);
                            }
                        }

                        patch.patched = true;
                    }
                }
            }
        }
    }

    /// Reset the installed flag for all patches associated with the active rom set.
    /// Should be called when rebooting emulated machine so that patches can be re-applied after ROMs
    /// are reloaded.
    pub fn reset_patches(&mut self) {
        let mut rom_str_vec = Vec::new();
        if let Some(rom_set) = &self.rom_set_active {
            for rom_str in &rom_set.roms {
                rom_str_vec.push(rom_str.clone());
            }
        }

        for rom_str in &rom_str_vec {
            if let Some(rom_desc) = self.get_romdesc_mut(rom_str) {
                for patch in &mut rom_desc.patches {
                    patch.patched = false;
                }
            }
        }   
    }

    pub fn is_patch_checkpoint(&self, address: u32) -> bool {
        let mut patch_checkpoint = false;

        if self.patches_active.get(&address).is_some() {
            patch_checkpoint = true;
        }
        patch_checkpoint
    }

    pub fn get_entrypoint(&self) -> (u16, u16) {
        if let Some(rom_set) = &self.rom_set_active {
            rom_set.reset_vector
        }
        else {
            (0xFFFF,0)
        }
    }

    pub fn get_checkpoint(&self, addr: u32) -> Option<&&str> {
        self.checkpoints_active.get(&addr)
    }

    pub fn get_available_features(&self) -> &Vec<RomFeature> {
        &self.features_available
    }

}