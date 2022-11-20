/*

    Machine.rs
    This module defines all the parts that make up the virtual computer.
    This module also contains the main run() method that makes the CPU execute instructions and
    run devices for a given time slice.

*/
use log;

use std::{
    rc::Rc,
    cell::{Cell, RefCell}, collections::VecDeque,
    fs::File,
    io::Write
};

use crate::{
    bus::{BusInterface, MemRangeDescriptor},
    cga::{self, CGACard},
    ega::{self, EGACard},
    vga::{self, VGACard},
    cpu::{self, CpuType, Cpu, Flag, CpuError},
    dma::{self, DMAControllerStringState},
    fdc::{self, FloppyController},
    hdc::{self, HardDiskController},
    floppy_manager::{FloppyManager},
    vhd_manager::{VHDManager},
    io::{IoHandler, IoBusInterface},
    mouse::Mouse,
    pit::{self, PitStringState},
    pic::{self, PicStringState},
    ppi::{self, PpiStringState},
    rom_manager::RomManager,
    serial::{self, SerialPortController},
    sound::{BUFFER_MS, VOLUME_ADJUST, SoundPlayer},
    videocard::{VideoCard, VideoType, VideoCardState}
};

use ringbuf::{RingBuffer, Producer, Consumer};

pub const NUM_FLOPPIES: u32 = 2;
pub const NUM_HDDS: u32 = 2;

pub const MAX_MEMORY_ADDRESS: usize = 0xFFFFF;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum MachineType {
    IBM_PC_5150,
    IBM_XT_5160
}

#[derive(Copy, Clone, Debug)]
pub enum ExecutionState {
    Paused,
    BreakpointHit,
    Running,
}

pub struct ExecutionControl {
    state: ExecutionState,
    do_step: Cell<bool>,
    do_run: Cell<bool>,
    do_reset: Cell<bool>
}

impl ExecutionControl {
    pub fn new() -> Self {
        Self { 
            state: ExecutionState::Paused,
            do_step: Cell::new(false), 
            do_run: Cell::new(false), 
            do_reset: Cell::new(false)
        }
    }

    pub fn set_state(&mut self, state: ExecutionState) {
        self.state = state
    }

    pub fn get_state(&self) -> ExecutionState {
        self.state
    }

    pub fn do_step(&mut self) {
        self.do_step.set(true);
    }

    pub fn do_run(&mut self) {
        // Run does nothing unless paused or at bp
        match self.state {
            ExecutionState::Paused => {
                self.do_run.set(true);
                self.state = ExecutionState::Running;
            }
            ExecutionState::BreakpointHit => {
                // Step out of breakpoint status into paused status
                self.do_run.set(true);
                self.state = ExecutionState::Running;
            }
            _ => {}
        }        
    }

    pub fn do_reset(&mut self) {
        self.do_reset.set(true)
    }
}
pub struct Machine {
    machine_type: MachineType,
    video_type: VideoType,
    sound_player: SoundPlayer,
    rom_manager: RomManager,
    floppy_manager: FloppyManager,
    //bus: BusInterface,
    io_bus: IoBusInterface,
    cpu: Cpu,
    dma_controller: Rc<RefCell<dma::DMAController>>,
    pit: Rc<RefCell<pit::Pit>>,
    pit_buffer_producer: Producer<u8>,
    pit_buffer_consumer: Consumer<u8>,
    pit_samples_produced: u64,
    pit_ticks_per_sample: f64,
    pit_ticks: f64,
    debug_snd_file: Option<File>,
    pic: Rc<RefCell<pic::Pic>>,
    ppi: Rc<RefCell<ppi::Ppi>>,
    video: Rc<RefCell<dyn VideoCard>>,
    fdc: Rc<RefCell<FloppyController>>,
    hdc: Rc<RefCell<HardDiskController>>,
    serial_controller: Rc<RefCell<serial::SerialPortController>>,
    mouse: Mouse,
    kb_buf: VecDeque<u8>,
    error: bool,
    error_str: String,
    cpu_cycles: u64,
}

impl Machine {
    pub fn new(
        machine_type: MachineType,
        video_type: VideoType,
        sound_player: SoundPlayer,
        rom_manager: RomManager,
        floppy_manager: FloppyManager,
        ) -> Machine {

        
        let mut io_bus = IoBusInterface::new();
        
        let mut cpu = Cpu::new(CpuType::Cpu8088);
        cpu.reset();        

        // Set up Ringbuffer for PIT channel #2 sampling for PC speaker
        let pit_buf_size = ((pit::PIT_MHZ * 1_000_000.0) * (BUFFER_MS as f64 / 1000.0)) as usize;
        let pit_buf: RingBuffer<u8> = RingBuffer::new(pit_buf_size);
        let (pit_buffer_producer, pit_buffer_consumer) = pit_buf.split();
        let sample_rate = sound_player.sample_rate();
        let pit_ticks_per_sample = (pit::PIT_MHZ * 1_000_000.0) / sample_rate as f64;

        // open a file to write the sound to
        //let mut debug_snd_file = File::create("output.pcm").expect("Couldn't open debug pcm file");
        
        log::trace!("Sample rate: {} pit_ticks_per_sample: {}", sample_rate, pit_ticks_per_sample);

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

        io_bus.register_port_handler(dma::DMA_CHANNEL_0_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_0_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_ADDR_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_WC_PORT, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_COMMAND_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_WRITE_REQ_REGISTER, IoHandler::new(dma.clone()));

        io_bus.register_port_handler(dma::DMA_CHANNEL_MASK_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_MODE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CLEAR_FLIPFLOP, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_MASTER_CLEAR, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CLEAR_MASK_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_WRITE_MASK_REGISTER, IoHandler::new(dma.clone()));

        io_bus.register_port_handler(dma::DMA_CHANNEL_0_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_1_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_2_PAGE_REGISTER, IoHandler::new(dma.clone()));
        io_bus.register_port_handler(dma::DMA_CHANNEL_3_PAGE_REGISTER, IoHandler::new(dma.clone()));

        // Floppy Controller:
        let mut fdc = Rc::new(RefCell::new(fdc::FloppyController::new()));
        io_bus.register_port_handler(fdc::FDC_DIGITAL_OUTPUT_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(fdc::FDC_STATUS_REGISTER, IoHandler::new(fdc.clone()));
        io_bus.register_port_handler(fdc::FDC_DATA_REGISTER, IoHandler::new(fdc.clone()));

        // Hard Disk Controller:  (Only functions if the required rom is loaded)
        let mut hdc = Rc::new(RefCell::new(hdc::HardDiskController::new(dma.clone(), hdc::DRIVE_TYPE2_DIP)));
        io_bus.register_port_handler(hdc::HDC_DATA_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_STATUS_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_READ_DIP_REGISTER, IoHandler::new(hdc.clone()));
        io_bus.register_port_handler(hdc::HDC_WRITE_MASK_REGISTER, IoHandler::new(hdc.clone()));

        // Serial Controller & Serial Ports
        let serial = Rc::new(RefCell::new(serial::SerialPortController::new()));
        io_bus.register_port_handler(serial::SERIAL1_RX_TX_BUFFER, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_INTERRUPT_ENABLE, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_INTERRUPT_ID, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_LINE_CONTROL, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_MODEM_CONTROL, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_LINE_STATUS, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL1_MODEM_STATUS, IoHandler::new(serial.clone()));

        io_bus.register_port_handler(serial::SERIAL2_RX_TX_BUFFER, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_INTERRUPT_ENABLE, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_INTERRUPT_ID, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_LINE_CONTROL, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_MODEM_CONTROL, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_LINE_STATUS, IoHandler::new(serial.clone()));
        io_bus.register_port_handler(serial::SERIAL2_MODEM_STATUS, IoHandler::new(serial.clone()));

        // Mouse
        let mouse = Mouse::new(serial.clone());

        // Initialize the appropriate model of Video Card.
        let mut video: Rc<RefCell<dyn VideoCard>> = match video_type {
            VideoType::CGA => {
                let video = Rc::new(RefCell::new(cga::CGACard::new()));
                io_bus.register_port_handlers(
                    vec![
                        cga::CRTC_REGISTER_SELECT,
                        cga::CRTC_REGISTER,
                        cga::CGA_MODE_CONTROL_REGISTER,
                        cga::CGA_COLOR_CONTROL_REGISTER,
                        cga::CGA_STATUS_REGISTER,
                        cga::CGA_LIGHTPEN_REGISTER,
                    ],
                    video.clone()
                );
                video
            }
            VideoType::EGA => {
                let video = Rc::new(RefCell::new(EGACard::new()));
                io_bus.register_port_handlers(
                    vec![
                        ega::MISC_OUTPUT_REGISTER,
                        ega::INPUT_STATUS_REGISTER_1,
                        ega::INPUT_STATUS_REGISTER_1_MDA,
                        ega::CRTC_REGISTER_ADDRESS,
                        ega::CRTC_REGISTER_ADDRESS_MDA,
                        ega::CRTC_REGISTER,
                        ega::CRTC_REGISTER_MDA,
                        ega::EGA_GRAPHICS_1_POSITION,
                        ega::EGA_GRAPHICS_2_POSITION, 
                        ega::EGA_GRAPHICS_ADDRESS,
                        ega::EGA_GRAPHICS_DATA,
                        ega::ATTRIBUTE_REGISTER,
                        ega::ATTRIBUTE_REGISTER_ALT,
                        ega::SEQUENCER_ADDRESS_REGISTER,
                        ega::SEQUENCER_DATA_REGISTER
                    ],
                    video.clone()
                );
                let mem_descriptor = MemRangeDescriptor::new(0xA0000, 65536, false );
                cpu.bus_mut().register_map(video.clone(), mem_descriptor);
                video
            }
            VideoType::VGA => {
                let video = Rc::new(RefCell::new(VGACard::new()));
                io_bus.register_port_handlers(
                    vec![
                        vga::MISC_OUTPUT_REGISTER_WRITE,
                        vga::MISC_OUTPUT_REGISTER_READ,
                        vga::INPUT_STATUS_REGISTER_1,
                        vga::INPUT_STATUS_REGISTER_1_MDA,
                        vga::CRTC_REGISTER_ADDRESS,
                        vga::CRTC_REGISTER_ADDRESS_MDA,
                        vga::CRTC_REGISTER,
                        vga::CRTC_REGISTER_MDA,
                        vga::GRAPHICS_ADDRESS,
                        vga::GRAPHICS_DATA,
                        vga::ATTRIBUTE_REGISTER,
                        vga::ATTRIBUTE_REGISTER_ALT,
                        vga::SEQUENCER_ADDRESS_REGISTER,
                        vga::SEQUENCER_DATA_REGISTER,
                        vga::PEL_ADDRESS_READ_MODE,
                        vga::PEL_ADDRESS_WRITE_MODE,
                        vga::PEL_DATA,    
                        vga::PEL_MASK,
                        vga::DAC_STATE_REGISTER,
                    ],
                    video.clone()
                );
                let mem_descriptor = MemRangeDescriptor::new(0xA0000, 65536, false );
                cpu.bus_mut().register_map(video.clone(), mem_descriptor);
                video
            }
            _=> panic!("Unsupported video card type.")
        };

        // Load BIOS ROM images
        rom_manager.copy_into_memory(cpu.bus_mut());

        // Set entry point for ROM (mostly used for diagnostic ROMs that don't have a FAR JUMP reset vector)
        let rom_entry_point = rom_manager.get_entrypoint();
        cpu.set_reset_address(rom_entry_point.0, rom_entry_point.1);
        cpu.reset_address();

        Machine {
            machine_type,
            video_type,
            sound_player,
            rom_manager,
            floppy_manager,
            //bus: bus,
            io_bus: io_bus,
            cpu: cpu,
            dma_controller: dma,
            pit: pit,
            pit_buffer_producer,
            pit_buffer_consumer,
            pit_ticks_per_sample,
            pit_ticks: 0.0,
            pit_samples_produced: 0,
            debug_snd_file: None,
            pic: pic,
            ppi: ppi,
            video: video,
            fdc: fdc,
            hdc: hdc,
            serial_controller: serial,
            mouse,
            kb_buf: VecDeque::new(),
            error: false,
            error_str: String::new(),
            cpu_cycles: 0
        }
    }

    pub fn bus(&self) -> &BusInterface {
        self.cpu.bus()
    }

    pub fn bus_mut(&mut self) -> &mut BusInterface {
        self.cpu.bus_mut()
    }

    //pub fn cga(&self) -> Rc<RefCell<CGACard>> {
    //    self.cga.clone()
    //}

    pub fn videocard(&self) -> Rc<RefCell<dyn VideoCard>> {
        self.video.clone()
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn fdc(&self) -> Rc<RefCell<FloppyController>> {
        self.fdc.clone()
    }

    pub fn hdc(&self) -> Rc<RefCell<HardDiskController>> {
        self.hdc.clone()
    }

    pub fn floppy_manager(&self) -> &FloppyManager {
        &self.floppy_manager
    }

    pub fn cpu_cycles(&self) -> u64 {
        self.cpu_cycles
    }

    pub fn pit_cycles(&self) -> u64 {
        self.pit.borrow().get_cycles()
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

    pub fn dma_state(&self) -> DMAControllerStringState {
        let dma = self.dma_controller.borrow();
        dma.get_string_state()
    }
    
    pub fn videocard_state(&self) -> VideoCardState {
        self.video.borrow().get_videocard_string_state()
    }

    pub fn get_error_str(&self) -> Option<&str> {
        match self.error {
            true => Some(&self.error_str),
            false => None
        }
    }

    pub fn key_press(&mut self, code: u8) {
        self.kb_buf.push_back(code);
    }

    pub fn key_release(&mut self, code: u8 ) {
        // HO Bit set converts a scancode into its 'release' code
        self.kb_buf.push_back(code | 0x80);
    }

    pub fn mouse(&self) -> &Mouse {
        &self.mouse
    }

    pub fn bridge_serial_port(&mut self, port_num: usize, port_name: String) {
        self.serial_controller.borrow_mut().bridge_port(port_num, port_name);
    }

    pub fn reset(&mut self) {
        self.cpu.reset();

        // Clear RAM
        self.cpu.bus_mut().reset();

        // Reload BIOS ROM images
        self.rom_manager.copy_into_memory(self.cpu.bus_mut());

        // Re-install ROM patches if any
        //self.rom_manager.install_patches(&mut self.bus);

        // Reset devices
        self.pit.borrow_mut().reset();
        self.pic.borrow_mut().reset();

        self.video.borrow_mut().reset();
    }

    fn cycles_to_us(&self, cycles: u32) -> f64 {

        1.0 / cpu::CPU_MHZ * cycles as f64
    }
    
    pub fn run(&mut self, cycle_target: u32, exec_control: &mut ExecutionControl, breakpoint: u32) {

        let mut kb_event_processed = false;

        // Was reset requested?
        if exec_control.do_reset.get() {
            self.reset();
            exec_control.do_reset.set(false);
            return
        }
    
        let mut ignore_breakpoint = false;
        let cycle_target_adj = match exec_control.state {
            ExecutionState::Paused => {
                match exec_control.do_step.get() {
                    true => {
                        // Reset step flag
                        exec_control.do_step.set(false);
                        ignore_breakpoint = true;
                        // Execute 1 cycle
                        1
                    },
                    false => return
                }
            }
            ExecutionState::Running => cycle_target,
            ExecutionState::BreakpointHit => cycle_target
        };

        let mut cycles_elapsed = 0;

        while cycles_elapsed < cycle_target_adj {

            let fake_cycles: u32 = 7;

            if self.cpu.is_error() == false {

                let flat_address = self.cpu.get_linear_ip();

                // Check for immediate breakpoint
                if (flat_address == breakpoint) && breakpoint != 0 && !ignore_breakpoint {

                    return
                }

                // Match checkpoints
                if let Some(cp) = self.rom_manager.get_checkpoint(flat_address) {
                    log::trace!("ROM CHECKPOINT: {}", cp);
                }

                // Check for patching checkpoint & install patches
                if self.rom_manager.is_patch_checkpoint(flat_address) {
                    log::trace!("ROM PATCH CHECKPOINT: Installing ROM patches");
                    self.rom_manager.install_patches(self.cpu.bus_mut());
                }

                match self.cpu.step(&mut self.io_bus) {
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
                            Some(irq) => {
                                self.cpu.do_hw_interrupt(irq);
                                self.cpu.resume();
                            },
                            None => {}
                        }
                    }
                }

                // Convert cycles into elapsed microseconds
                let us = self.cycles_to_us(fake_cycles);

                // Process a keyboard event once per frame.
                // A reasonably fast typist can generate two events in a single 16ms frame, and to the virtual cpu
                // they then appear to happen instantenously. The PPI has no buffer, so one scancode gets lost. 
                // 
                // If we limit keyboard events to once per frame, this avoids this problem. I'm a reasonably
                // fast typist and this method seems to work fine.
                if self.kb_buf.len() > 0 && !kb_event_processed {

                    let kb_byte = self.kb_buf.pop_front().unwrap();

                    self.ppi.borrow_mut().send_keyboard(kb_byte);
                    self.pic.borrow_mut().request_interrupt(1);
                    kb_event_processed = true;
                }

                // Run devices
                
                self.dma_controller.borrow_mut().run(&mut self.io_bus);

                // PIT needs PIC to issue timer interrupts, DMA to do DRAM refresh, PPI for timer gate & speaker data
                self.pit.borrow_mut().run(
                    self.cpu.bus_mut(),
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    &mut self.ppi.borrow_mut(),
                    &mut self.pit_buffer_producer,
                    fake_cycles);

                // Sample the PIT channel
                self.pit_ticks += fake_cycles as f64;
                while self.pit_ticks >= self.pit_ticks_per_sample {
                    self.pit_buf_to_sound_buf();
                    self.pit_ticks -= self.pit_ticks_per_sample;
                }

                //while self.pit_buffer_consumer.len() >= self.pit_ticks_per_sample as usize {
                //    self.pit_buf_to_sound_buf();
                //}

                // Run the video device
                // This uses dynamic dispatch - be aware of any performance hit
                self.video.borrow_mut().run( fake_cycles);
                
                self.ppi.borrow_mut().run(&mut self.pic.borrow_mut(), fake_cycles);
                
                // FDC needs PIC to issue controller interrupts, DMA to request DMA transfers, and Memory Bus to read/write to via DMA
                self.fdc.borrow_mut().run(
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    self.cpu.bus_mut(),
                    fake_cycles);

                // HDC needs PIC to issue controller interrupts, DMA to request DMA stransfers, and Memory Bus to read/write to via DMA                    
                self.hdc.borrow_mut().run(
                    &mut self.pic.borrow_mut(),
                    &mut self.dma_controller.borrow_mut(),
                    self.cpu.bus_mut(),
                    fake_cycles);         
                    
                // Serial port needs PIC to issue interrupts
                self.serial_controller.borrow_mut().run(
                    &mut self.pic.borrow_mut(),
                    us);

                self.mouse.run(us);
            }
            // Eventually we want to return per-instruction cycle counts, emulate the effect of PIQ, DMA, wait states, all
            // that good stuff. For now during initial development we're going to assume an average instruction cost of 8** 7
            // even cycles keeps the BIOS PIT test from working!
            cycles_elapsed += fake_cycles;
            self.cpu_cycles += fake_cycles as u64;
        }
    }

    /// Called to update machine once per frame.
    /// Mostly used for serial function.
    pub fn frame_update(&mut self) {

        self.serial_controller.borrow_mut().update();
    }

    pub fn play_sound_buffer(&self) {
        self.sound_player.play();
    }

    pub fn pit_buf_to_sound_buf(&mut self) {

        let pit_ticks: usize = self.pit_ticks_per_sample as usize;
        if self.pit_buffer_consumer.len() < pit_ticks {
            return
        }

        let mut sum = 0;
        let mut sample = 0;
        for _ in 0..pit_ticks {
            
            sample = match self.pit_buffer_consumer.pop() {
                Some(s) => s,
                None => {
                    log::trace!("No byte in pit buffer");
                    0
                }
            };
            sum += sample;
        }

        let average: f32 = sum as f32 / pit_ticks as f32;

        //log::trace!("Sample: sum: {}, ticks: {}, avg: {}", sum, pit_ticks, average);

        self.pit_samples_produced += 1;
        //log::trace!("producer: {}", self.pit_samples_produced);

        self.sound_player.queue_sample(average as f32 * VOLUME_ADJUST);
        //self.debug_snd_file.write(&average.to_be_bytes()).expect("Error writing to debug sound file");
                
    }
}