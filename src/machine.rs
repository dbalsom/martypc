
use log;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use crate::{
    bus::BusInterface,
    dma,
    cga::{self, CGACard},
    cpu::{Cpu, Flag, CpuError},
    floppy,
    io::{IoHandler, IoBusInterface},
    pit::{self, PitStringState},
    pic::{self, PicStringState},
    ppi::{self, PpiStringState},
};

pub const MAX_MEMORY_ADDRESS: usize = 0xFFFFF;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum MachineType {
    IBM_PC_5150,
    IBM_XT_5160
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum VideoType {
    MDA,
    CGA,
    EGA,
    VGA
}

pub struct Machine {
    machine_type: MachineType,
    video_type: VideoType,
    bus: BusInterface,
    io_bus: IoBusInterface,
    cpu: Cpu,
    dma_controller: Rc<RefCell<dma::DMAController>>,
    pit: Rc<RefCell<pit::Pit>>,
    pic: Rc<RefCell<pic::Pic>>,
    ppi: Rc<RefCell<ppi::Ppi>>,
    cga: Rc<RefCell<cga::CGACard>>,
    error: bool,
    error_str: String,

}

lazy_static! {
    static ref BIOS_CHECKPOINTS: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(0xfe01a, "RAM Check Routine");
        m.insert(0xfe05b, "8088 Processor Test");
        m.insert(0xfe0b0, "ROS Checksum");
        m.insert(0xfe0da, "8237 DMA Initialization Test");
        m.insert(0xfe117, "DMA Controller test");
        m.insert(0xfe158, "Base 16K Read/Write Test");
        m.insert(0xfe235, "8249 Interrupt Controller Test");
        m.insert(0xfe285, "8253 Timer Checkout");
        m.insert(0xfe33b, "ROS Checksum II");
        m.insert(0xfe352, "Initialize CRTC Controller");
        m.insert(0xfe3af, "Video Line Test");
        m.insert(0xfe4c7, "Keyboard Test");
        m.insert(0xfe3c0, "CRT Interface Lines Test");
        m.insert(0xfe55c, "Diskette Attachment Test");
        m.insert(0xfe630, "Error Beep");
        m.insert(0xfe666, "Beep");
        m.insert(0xfe688, "Keyboard Reset");
        m.insert(0xfe6b2, "Blink LED Interrupt");
        m.insert(0xfe6ca, "Print Message");
        m.insert(0xfe6f2, "Bootstrap Loader");
        m.insert(0xf6000, "ROM BASIC");
        m
    };
}

impl Machine {
    pub fn new(
        machine_type: MachineType,
        video_type: VideoType,
        bios_buf: Vec<u8>,
        basic_buf: Vec<u8>) -> Machine {

        let mut bus = BusInterface::new();
        let mut io_bus = IoBusInterface::new();
        
        let mut cpu = Cpu::new(4);
        cpu.reset();        

        // Attach IO Device handlers

        // Intel 8259 Programmable Interrupt Controller
        let mut pic = Rc::new(RefCell::new(pic::Pic::new()));
        io_bus.register_port_handler(pic::PIC_COMMAND_PORT, IoHandler::new(pic.clone()));
        io_bus.register_port_handler(pic::PIC_DATA_PORT, IoHandler::new(pic.clone()));

        // Intel 8255 Programmable Peripheral Interface
        // PPI Needs to know machine_type as DIP switches and thus PPI behavior are different 
        // for PC vs XT
        let mut ppi = Rc::new(RefCell::new(ppi::Ppi::new(machine_type, video_type)));
        io_bus.register_port_handler(ppi::PPI_PORT_A, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_PORT_B, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_PORT_C, IoHandler::new(ppi.clone()));
        io_bus.register_port_handler(ppi::PPI_COMMAND_PORT, IoHandler::new(ppi.clone()));
        
        // Intel 8253 Programmable Interval Timer
        // Ports 0x40,41,42 Data ports, 0x43 Control port
        let mut pit = Rc::new(RefCell::new(pit::ProgrammableIntervalTimer::new()));
        io_bus.register_port_handler(pit::PIT_COMMAND_REGISTER, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_0_DATA_PORT, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_1_DATA_PORT, IoHandler::new(pit.clone()));
        io_bus.register_port_handler(pit::PIT_CHANNEL_2_DATA_PORT, IoHandler::new(pit.clone()));

        // DMA Controller: 
        // Intel 8237 DMA Controller
        let mut dma = Rc::new(RefCell::new(dma::DMAController::new()));
        io_bus.register_port_handler(dma::DMA_CONTROL_PORT, IoHandler::new(dma.clone()));

        // Floppy Controller:
        let mut fdc = Rc::new(RefCell::new(floppy::FloppyController::new()));
        io_bus.register_port_handler(floppy::FDC_DIGITAL_OUTPUT_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(floppy::FDC_STATUS_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(floppy::FDC_DATA_REGISTER, IoHandler::new(fdc.clone()));

        // CGA card
        let mut cga = Rc::new(RefCell::new(cga::CGACard::new()));
        io_bus.register_port_handler(cga::CRTC_REGISTER_SELECT, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CRTC_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_MODE_CONTROL_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_STATUS_REGISTER, IoHandler::new(cga.clone()));
        io_bus.register_port_handler(cga::CGA_LIGHTPEN_REGISTER, IoHandler::new(cga.clone()));

        // Install BIOS image
        bus.copy_from(bios_buf, 0xFE000, 4, true).unwrap();

        // Load ROM BASIC if present
        if basic_buf.len() > 0 {
            bus.copy_from(basic_buf, 0xF6000, 4, true).unwrap();
        }

        // Temporarily patch DMA test
        bus.patch_from(vec![0xEB, 0x03], 0xFE130).unwrap();  // JZ -> JNP
        // Patch Checksum test since we patched BIOS
        bus.patch_from(vec![0x74, 0xD5], 0xFE0D8).unwrap();  // JNZ -> JZ

        Machine {
            machine_type: machine_type,
            video_type: video_type,
            bus: bus,
            io_bus: io_bus,
            cpu: cpu,
            dma_controller: dma,
            pit: pit,
            pic: pic,
            ppi: ppi,
            cga: cga,
            error: false,
            error_str: String::new(),
        }
    }

    pub fn bus(&self) -> &BusInterface {
        &self.bus
    }

    pub fn mut_bus(&mut self) -> &mut BusInterface {
        &mut self.bus
    }

    pub fn cga(&self) -> Rc<RefCell<CGACard>> {
        self.cga.clone()
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn pit_state(&self) -> PitStringState {
        let pit = self.pit.borrow();
        let pit_data = pit.get_string_repr();
        pit_data
    }

    pub fn pic_state(&self) -> PicStringState {
        let pic = self.pic.borrow();
        pic.get_string_state()
    }

    pub fn ppi_state(&self) -> PpiStringState {
        let pic = self.ppi.borrow();
        pic.get_string_state()
    }

    pub fn get_error_str(&self) -> Option<&str> {
        match self.error {
            true => Some(&self.error_str),
            false => None
        }
    }

    pub fn key_press(&self, code: u8) {
        self.ppi.borrow_mut().send_keyboard(code);
        self.pic.borrow_mut().request_interrupt(1);
    }

    pub fn key_release(&self, code: u8 ) {
        // HO Bit set converts a scancode into its 'release' code
        self.ppi.borrow_mut().send_keyboard(code | 0x80);
        self.pic.borrow_mut().request_interrupt(1);
    }

    pub fn run(&mut self, cycle_target: u32, single_step: bool, breakpoint: u32) {

        let cycle_target_adj = match single_step {
            true => 1,
            false => cycle_target
        };

        let mut cycles_elapsed = 0;

        while cycles_elapsed < cycle_target_adj {
            if self.cpu.is_error() == false {

                // Match checkpoints
                let flat_address = self.cpu.get_flat_address();
                match BIOS_CHECKPOINTS.get(&flat_address) {
                    Some(str) => log::trace!("BIOS CHECKPOINT: {}", str),
                    None => {}
                }

                // Check for breakpoint
                if flat_address == breakpoint && breakpoint != 0 {
                    return
                }

                match self.cpu.step(&mut self.bus, &mut self.io_bus) {
                    Ok(()) => {
                    },
                    Err(err) => {
                        self.error = true;
                        self.error_str = format!("{}", err);
                        log::error!("CPU Error: {}\n{}", err, self.cpu.dump_instruction_history());
                    } 
                }

                // Check for hardware interrupts if Interrupt Flag is set and not in wait cycle
                if self.cpu.interrupts_enabled() {

                    let mut pic = self.pic.borrow_mut();
                    if pic.query_interrupt_line() {
                        match pic.get_interrupt_vector() {
                            Some(irq) =>  self.cpu.do_hw_interrupt(&mut self.bus, irq),
                            None => {}
                        }
                    }
                }

                // Run devices
                self.dma_controller.borrow_mut().run(&mut self.io_bus);
                self.pit.borrow_mut().run(&mut self.io_bus,&mut self.pic.borrow_mut(), 7);
                self.cga.borrow_mut().run(&mut self.io_bus, 7);
                self.ppi.borrow_mut().run(&mut self.pic.borrow_mut(), 7);

            }
            // Eventually we want to return per-instruction cycle counts, emulate the effect of PIQ, DMA, wait states, all
            // that good stuff. For now during initial development we're going to assume an average instruction cost of 8** 7
            // even cycles keeps the BIOS PIT test from working!
            cycles_elapsed += 7;
        }

    }
}