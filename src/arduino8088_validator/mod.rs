/*
  Marty PC Emulator 
  (C)2023 Daniel Balsom
  https://github.com/dbalsom/marty

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use crate::cpu_validator::{CpuValidator, VRegisters, ReadType};

mod queue;
mod udmask;

use crate::arduino8088_client::*;
use crate::arduino8088_validator::queue::*;

const ADDRESS_SPACE: usize = 1_048_576;

const VISIT_ONCE: bool = false;
const NUM_INVALID_FETCHES: usize = 6;
const NUM_MEM_OPS: usize = 0x20000 + 16;
const V_INVALID_POINTER: u32 = 0xFFFFFFFF;
const UPPER_MEMORY: u32 = 0xA0000;
const CYCLE_LIMIT: u32 = 1000;

pub const MOF_UNUSED: u8 = 0x00;
pub const MOF_EMULATOR: u8 = 0x01;
pub const MOF_PI8088: u8 = 0x02;

const DATA_PROGRAM: u8 = 0;
const DATA_FINALIZE: u8 = 1;

const OPCODE_NOP: u8 = 0x90;

#[derive (Default, Debug)]
pub struct RemoteCpuRegisters {
    
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    ss: u16,
    ds: u16,
    es: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    cs: u16,
    ip: u16,
    flags: u16
}

#[derive (PartialEq)]
pub enum BusCycle {
    T1,
    T2,
    T3,
    T4,
    Tw
}

#[derive (PartialEq, Debug)]
pub enum ValidatorState {
    Setup,
    Execute,
    Readback,
    Finished
}

#[derive (PartialEq, Debug)]
pub enum AccessType {
    AccAlternateData = 0x0,
    AccStack,
    AccCodeOrNone,
    AccData,
}

#[derive (Copy, Clone, PartialEq)]
pub enum BusOpType {
    CodeRead,
    MemRead,
    MemWrite,
    IoRead,
    IoWrite,
}

#[derive (Copy, Clone)]
pub struct BusOp {
    op_type: BusOpType,
    addr: u32,
    data: u8,
    flags: u8
}

#[derive (Default)]
pub struct Frame {
    name: String,
    instr: Vec<u8>,
    opcode: u8,
    modrm: u8,
    has_modrm: bool,
    discard: bool,
    next_fetch: bool,
    num_nop: i32,

    regs: Vec<VRegisters>,

    prefetch_addr: Vec<u32>,
    
    emu_prefetch: Vec<BusOp>,
    emu_ops: Vec<BusOp>,
    cpu_prefetch: Vec<BusOp>,
    cpu_ops: Vec<BusOp>,
    mem_op_n: usize
}

impl Frame {
    pub fn new() -> Self {

        Self {
            name: "NewFrame".to_string(),
            instr: Vec::new(),
            opcode: 0,
            modrm: 0,
            has_modrm: false,
            discard: false,
            next_fetch: false,
            num_nop: 0,
            regs: vec![VRegisters::default(); 2],

            prefetch_addr: vec![0; NUM_INVALID_FETCHES],

            emu_prefetch: Vec::new(),
            emu_ops: Vec::new(),
            cpu_prefetch: Vec::new(),
            cpu_ops: Vec::new(),
            mem_op_n: 0,
        }
    }
}

pub struct RemoteCpu {
    cpu_client: CpuClient,
    regs: RemoteCpuRegisters,
    memory: Vec<u8>,
    pc: usize,
    end_addr: usize,
    program_state: ProgramState,

    address_latch: u32,
    status: u8,
    command_status: u8,
    control_status: u8,
    data_bus: u8,
    data_type: QueueDataType,

    cycle_num: u32,
    bus_state: BusState,
    bus_cycle: BusCycle,

    queue: InstructionQueue,
    queue_byte: u8,
    queue_type: QueueDataType,
    queue_first_fetch: bool,
    queue_fetch_n: u8,
    opcode: u8,
    finalize: bool,

    // Validator stuff
    busop_n: usize,
    prefetch_n: usize,
    v_pc: usize,
}

impl RemoteCpu {
    pub fn new(cpu_client: CpuClient) -> RemoteCpu {
        RemoteCpu {
            cpu_client,
            regs: Default::default(),
            memory: vec![0; ADDRESS_SPACE],
            pc: 0,
            end_addr: 0,
            program_state: ProgramState::Reset,
            address_latch: 0,
            status: 0,
            command_status: 0,
            control_status: 0,
            data_bus: 0,
            data_type: QueueDataType::Program,
            cycle_num: 0,
            bus_state: BusState::PASV,
            bus_cycle: BusCycle::T1,
            queue: InstructionQueue::new(),
            queue_byte: 0,
            queue_type: QueueDataType::Program,
            queue_first_fetch: true,
            queue_fetch_n: 0,
            opcode: 0,
            finalize: false,

            busop_n: 0,
            prefetch_n: 0,
            v_pc: 0,
        }
    }
}

pub struct ArduinoValidator {

    //cpu_client: Option<CpuClient>,
    cpu: RemoteCpu,

    current_frame: Frame,
    state: ValidatorState,

    cycle_count: u64,

    rd_signal: bool,
    wr_signal: bool,
    iom_signal: bool,
    ale_signal: bool,

    address_latch: u32,

    cpu_memory_access: AccessType,
    cpu_interrupt_enabled: bool,

    scratchpad: Vec<u8>,
    code_as_data_skip: bool,
    readback_ptr: usize,
    trigger_addr: u32,

    mask_flags: bool,

    visit_once: bool,
    visited: Vec<bool>,

    log_prefix: String
}

impl ArduinoValidator {

    pub fn new() -> Self {

        // Trigger addr is address at which to start validation
        // if trigger_addr == V_INVALID_POINTER then validate        
        let trigger_addr = V_INVALID_POINTER;

        let cpu_client = match CpuClient::init() {
            Ok(client) => client,
            Err(e) => {
                panic!("Failed to initialize ArduinoValidator");
            }
        };

        ArduinoValidator {
            cpu: RemoteCpu::new(cpu_client),

            current_frame: Frame::new(),
            state: ValidatorState::Setup,

            cycle_count: 0,
            rd_signal: false,
            wr_signal: false, 
            iom_signal: false,
            ale_signal: false,   
            address_latch: 0,
            cpu_memory_access: AccessType::AccAlternateData,
            cpu_interrupt_enabled: false,

            scratchpad: vec![0; 0x100000],
            code_as_data_skip: false,
            readback_ptr: 0,
            trigger_addr,
            mask_flags: true,
            visit_once: VISIT_ONCE,
            visited: vec![false; 0x100000],

            log_prefix: String::new()
        }
    }

    pub fn regs_to_buf(buf: &mut [u8], regs: &VRegisters) {
        // AX, BX, CX, DX, SS, SP, FLAGS, IP, CS, DS, ES, BP, SI, DI
        buf[0] = (regs.ax & 0xFF) as u8;
        buf[1] = ((regs.ax >> 8) & 0xFF) as u8;

        buf[2] = (regs.bx & 0xFF) as u8;
        buf[3] = ((regs.bx >> 8) & 0xFF) as u8;

        buf[4] = (regs.cx & 0xFF) as u8;
        buf[5] = ((regs.cx >> 8) & 0xFF) as u8;
        
        buf[6] = (regs.dx & 0xFF) as u8;
        buf[7] = ((regs.dx >> 8) & 0xFF) as u8;        

        buf[8] = (regs.ss & 0xFF) as u8;
        buf[9] = ((regs.ss >> 8) & 0xFF) as u8;
        
        buf[10] = (regs.sp & 0xFF) as u8;
        buf[11] = ((regs.sp >> 8) & 0xFF) as u8;
        
        buf[12] = (regs.flags & 0xFF) as u8;
        buf[13] = ((regs.flags >> 8) & 0xFF) as u8;       
        
        buf[14] = (regs.ip & 0xFF) as u8;
        buf[15] = ((regs.ip >> 8) & 0xFF) as u8;
        
        buf[16] = (regs.cs & 0xFF) as u8;
        buf[17] = ((regs.cs >> 8) & 0xFF) as u8;
        
        buf[18] = (regs.ds & 0xFF) as u8;
        buf[19] = ((regs.ds >> 8) & 0xFF) as u8;
        
        buf[20] = (regs.es & 0xFF) as u8;
        buf[21] = ((regs.es >> 8) & 0xFF) as u8;
        
        buf[22] = (regs.bp & 0xFF) as u8;
        buf[23] = ((regs.bp >> 8) & 0xFF) as u8;
        
        buf[24] = (regs.si & 0xFF) as u8;
        buf[25] = ((regs.si >> 8) & 0xFF) as u8;
        
        buf[26] = (regs.di & 0xFF) as u8;
        buf[27] = ((regs.di >> 8) & 0xFF) as u8;
    }

    pub fn buf_to_regs(buf: &[u8]) -> VRegisters {
        VRegisters {
            ax: buf[0] as u16 | ((buf[1] as u16) << 8),
            bx: buf[2] as u16 | ((buf[3] as u16) << 8),
            cx: buf[4] as u16 | ((buf[5] as u16) << 8),
            dx: buf[6] as u16 | ((buf[7] as u16) << 8),
            ss: buf[8] as u16 | ((buf[9] as u16) << 8),
            sp: buf[10] as u16 | ((buf[11] as u16) << 8),
            flags: buf[12]  as u16| ((buf[13] as u16) << 8),
            ip: buf[14] as u16 | ((buf[15] as u16) << 8),
            cs: buf[16] as u16 | ((buf[17] as u16) << 8),
            ds: buf[18] as u16 | ((buf[19] as u16) << 8),
            es: buf[20] as u16 | ((buf[21] as u16) << 8),
            bp: buf[22] as u16 | ((buf[23] as u16) << 8),
            si: buf[24] as u16 | ((buf[25] as u16) << 8),
            di: buf[26] as u16| ((buf[27] as u16) << 8),
        }
    }

    pub fn validate_mem_ops(&mut self) {

        if self.current_frame.emu_ops.len() != self.current_frame.cpu_ops.len() {
            log::error!(
                "Validator error: Memory op count mismatch. Emu: {} CPU: {}", 
                self.current_frame.emu_ops.len(),
                self.current_frame.cpu_ops.len()
            );
        }
    }

    pub fn validate_registers(&mut self) {

        let mut reg_buf: [u8; 28] = [0; 28];

        match self.cpu.cpu_client.store_registers_to_buf(&mut reg_buf) {
            Ok(_) => {},
            Err(e) => {
                log::error!("Validator error: Store registers failed.");
                return
            }
        }        

        let regs = ArduinoValidator::buf_to_regs(&mut reg_buf);

        assert_eq!(self.current_frame.regs[1].ax, regs.ax);
        assert_eq!(self.current_frame.regs[1].bx, regs.bx);
        assert_eq!(self.current_frame.regs[1].cx, regs.cx);
        assert_eq!(self.current_frame.regs[1].dx, regs.dx);
        assert_eq!(self.current_frame.regs[1].cs, regs.cs);
        assert_eq!(self.current_frame.regs[1].ss, regs.ss);
        assert_eq!(self.current_frame.regs[1].ds, regs.ds);
        assert_eq!(self.current_frame.regs[1].es, regs.es);
        assert_eq!(self.current_frame.regs[1].sp, regs.sp);
        assert_eq!(self.current_frame.regs[1].bp, regs.bp);
        assert_eq!(self.current_frame.regs[1].si, regs.si);
        assert_eq!(self.current_frame.regs[1].bp, regs.bp);

        let mut emu_flags_masked = self.current_frame.regs[1].flags;
        let mut cpu_flags_masked = regs.flags;

        if self.mask_flags {
            emu_flags_masked = ArduinoValidator::mask_undefined_flags(self.current_frame.opcode, self.current_frame.modrm, self.current_frame.regs[1].flags);
            cpu_flags_masked = ArduinoValidator::mask_undefined_flags(self.current_frame.opcode, self.current_frame.modrm, regs.flags);
        }

        if emu_flags_masked != cpu_flags_masked {
            log::error!("CPU flags mismatch! EMU: 0b{:08b} != CPU: 0b{:08b}", emu_flags_masked, cpu_flags_masked);
            panic!("CPU flag mismatch!")
        }
    }
}

impl RemoteCpu {

    pub fn update_state(&mut self) -> bool {
        self.program_state = self.cpu_client.get_program_state().expect("Failed to get program state!");
        self.status = self.cpu_client.read_status().expect("Failed to get status!");
        self.command_status = self.cpu_client.read_8288_command().expect("Failed to get 8288 command status!");
        self.control_status = self.cpu_client.read_8288_control().expect("Failed to get 8288 control status!");
        self.data_bus = self.cpu_client.read_data_bus().expect("Failed to get data bus!");
        true    
    }

    pub fn cycle(
        &mut self,
        instr: &[u8],
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>, 
        cpu_mem_ops: &mut Vec<BusOp>
    ) -> bool {

        self.cpu_client.cycle().expect("Failed to cycle cpu!");
        
        self.bus_cycle = match self.bus_cycle {
            BusCycle::T1 => {
                // Capture the state of the bus transfer in T1, as the state will go PASV in t3-t4
                self.bus_state = get_bus_state!(self.status);
                
                // Only exit T1 state if bus transfer state indicates a bus transfer
                match get_bus_state!(self.status) {
                    BusState::PASV => BusCycle::T1,
                    BusState::HALT => BusCycle::T1,
                    _ => BusCycle::T2
                }
            }
            BusCycle::T2 => {
                BusCycle::T3
            }
            BusCycle::T3 => {
                // TODO: Handle wait states
                BusCycle::T4
            }
            BusCycle::Tw => {
                // TODO: Handle wait states
                BusCycle::T4
            }            
            BusCycle::T4 => {
                if self.bus_state == BusState::CODE {
                    // We completed a code fetch, so add to prefetch queue
                    self.queue.push(self.data_bus, self.data_type);
                }
                BusCycle::T1
            }            
        };

        self.update_state();

        if(self.command_status & COMMAND_ALE_BIT) != 0 {
            if self.bus_cycle != BusCycle::T1 {
                log::warn!("ALE on non-T1 cycle state! CPU desynchronized.");
                self.bus_cycle = BusCycle::T1;
            }

            self.address_latch = self.cpu_client.read_address_latch().expect("Failed to get address latch!");
        }

        // Do reads & writes if we are in execute state.
        if self.program_state == ProgramState::Execute {
            // MRDC status is active-low.
            if (self.command_status & COMMAND_MRDC_BIT) == 0 {
                // CPU is reading from bus.

                if self.bus_state == BusState::CODE {
                    // CPU is reading code.
                    if self.v_pc < instr.len() {
                        // Feed current instruction to CPU
                        log::trace!("CPU fetch");
                        self.data_bus = instr[self.v_pc];
                        self.data_type = QueueDataType::Program;
                        self.v_pc += 1;
                    }
                    else {
                        // Fetch past end of instruction. Send NOP
                        self.data_bus = OPCODE_NOP;
                        self.data_type = QueueDataType::Finalize;
                    }
                }
                if self.bus_state == BusState::MEMR {
                    // CPU is reading data from memory.
                    if self.busop_n > emu_mem_ops.len() {
                        
                        log::trace!("CPU read");
                        assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemRead);
                        // Feed emulator byte to CPU
                        self.data_bus = emu_mem_ops[self.busop_n].data;
                        // Add emu op to CPU BusOp list
                        cpu_mem_ops.push(emu_mem_ops[self.busop_n].clone());
                        self.busop_n += 1;
                    }                    
                }
                self.data_bus = self.memory[self.address_latch as usize];
                self.cpu_client.write_data_bus(self.data_bus).expect("Failed to write data bus.");
            }

            // MWTC status is active-low.
            if (self.command_status & COMMAND_MWTC_BIT) == 0 {
                // CPU is writing to bus.
                
                if self.bus_state == BusState::MEMW {
                    // CPU is writing data to memory.
                    if self.busop_n > emu_mem_ops.len() {

                        assert!(emu_mem_ops[self.busop_n].op_type == BusOpType::MemWrite);
                        log::trace!("CPU write");

                        // Read byte from CPU
                        self.data_bus = self.cpu_client.read_data_bus().expect("Failed to read data bus.");
                        
                        // Add write op to CPU BusOp list
                        cpu_mem_ops.push(
                            BusOp {
                                op_type: BusOpType::MemWrite,
                                addr: self.address_latch,
                                data: self.data_bus,
                                flags: 0
                            }
                        );
                        self.busop_n += 1;
                    }                   
                }
            }

            // IORC status is active-low.
            if (self.command_status & COMMAND_IORC_BIT) == 0 {
                // CPU is reading from IO address.
                log::trace!("validator: Unhandled IO op");
            }

            // IOWC status is active-low.
            if (self.command_status & COMMAND_IOWC_BIT) == 0 {
                // CPU is writing to IO address.
                log::trace!("validator: Unhandled IO op");
            }
        }



        // Handle queue activity
        let q_op = get_queue_op!(self.status);
            
        match q_op {
            QueueOp::First | QueueOp::Subsequent => {
                // We fetched a byte from the queue last cycle
                (self.queue_byte, self.queue_type ) = self.queue.pop();
                if q_op == QueueOp::First {
                    // First byte of instruction fetched.
                    self.queue_first_fetch = true;
                    self.queue_fetch_n = 0;
                    self.opcode = self.queue_byte;
                }
                else {
                    // Subsequent byte of instruction fetched
                    self.queue_fetch_n += 1;
                }
            }
            QueueOp::Flush => {
                // Queue was flushed last cycle
                self.queue.flush();
            }
            _ => {}
        }

        self.cycle_num += 1;
        true        
    }

    pub fn run(
        &mut self, 
        instr: &[u8],
        emu_prefetch: &Vec<BusOp>,
        emu_mem_ops: &Vec<BusOp>,
        cpu_prefetch: &mut Vec<BusOp>, 
        cpu_mem_ops: &mut Vec<BusOp>
    ) -> Result<bool, CpuClientError> {
    

        self.busop_n = 0;
        self.prefetch_n = 0;
        self.v_pc = 0;

        self.address_latch = self.cpu_client.read_address_latch().expect("Failed to get address latch!");

        self.update_state();

        // ALE should be active at start of execution
        if self.command_status & COMMAND_ALE_BIT == 0 {
            log::warn!("Execution is not starting on T1.");
        }

        //self.print_cpu_state();

        let v_pc: usize = 0;

        while self.cycle_num < CYCLE_LIMIT {
            self.cycle(instr, emu_prefetch, emu_mem_ops, cpu_prefetch, cpu_mem_ops);
            //self.print_cpu_state();

            if self.bus_state == BusState::CODE && (self.v_pc >= instr.len()) {
                // We are fetching past the end of the current instruction. Finalize execution.
                if self.program_state == ProgramState::Execute {
                    self.cpu_client.finalize().expect("Failed to finalize!");
    
                    // Wait for execution to finalize
                    while self.program_state != ProgramState::ExecuteDone {
                        self.cycle(instr, emu_prefetch, emu_mem_ops, cpu_prefetch, cpu_mem_ops);
                    }
    
                    // Program finalized!
                    log::trace!("Program finalized! Run store now.");

                    return Ok(true);
                }
            }
        }

        // Ran past cycle limit
        Ok(false)
    }

    pub fn load(&mut self, reg_buf: &[u8]) -> Result<bool, CpuClientError> {

        self.cpu_client.load_registers_from_buf(&reg_buf)?;
        Ok(true)
    }

    pub fn store(&mut self) -> Result<bool, CpuClientError> {

        let mut buf: [u8; 28] = [0; 28];
        self.cpu_client.store_registers_to_buf(&mut buf)?;

        let regs = ArduinoValidator::buf_to_regs(&buf);
        //RemoteCpu::print_regs(&regs);
        Ok(true)
    }    
}

pub fn make_pointer(base: u16, offset: u16) -> u32 {
    return (((base as u32) << 4) + offset as u32 ) & 0xFFFFF;
}

impl CpuValidator for ArduinoValidator {

    fn init(&mut self, mask_flags: bool) -> bool {

        self.mask_flags = mask_flags;
        true
    }

    fn begin(&mut self, regs: &VRegisters ) {

        self.current_frame.discard = false;
        self.current_frame.regs[0] = regs.clone();

        let ip_addr = make_pointer(regs.cs, regs.ip);

        //println!("{} : {}", self.trigger_addr, ip_addr);
        if self.trigger_addr == ip_addr {
            log::info!("Trigger address hit, begin validation...");
            self.trigger_addr = V_INVALID_POINTER;
        }

        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY && self.visited[ip_addr as usize]) {
            self.current_frame.discard = true;
            return;
        }

        self.current_frame.emu_ops.clear();
        self.current_frame.emu_prefetch.clear();
        self.current_frame.cpu_ops.clear();
        self.current_frame.cpu_prefetch.clear();
    }    

    fn validate(&mut self, name: String, instr: &[u8], has_modrm: bool, cycles: i32, regs: &VRegisters) {

        let ip_addr = make_pointer(self.current_frame.regs[0].cs, self.current_frame.regs[0].ip);

        if (self.trigger_addr != V_INVALID_POINTER) 
            || (self.visit_once && ip_addr >= UPPER_MEMORY &&  self.visited[ip_addr as usize]) {
            return
        }

        self.visited[ip_addr as usize] = true;

        self.current_frame.name = name.clone();
        self.current_frame.opcode = instr[0];
        self.current_frame.instr = instr.to_vec();
        self.current_frame.has_modrm = has_modrm;
        self.current_frame.num_nop = 0;
        self.current_frame.next_fetch = false;
        self.current_frame.regs[1] = *regs;

        if has_modrm {
            if instr.len() < 2 {
                log::error!("validate(): modrm specified but instruction length < 2");
                return
            }
            self.current_frame.modrm = instr[1];
        }
        else {
            self.current_frame.modrm = 0;
        }

        // We must discard the first instructions after boot.
        /*
        if ((ip_addr >= 0xFFFF0) || (ip_addr <= 0xFF)) {
            log::debug!("Instruction out of range: Discarding...");
            self.current_frame.discard = true;
        }
        */

        let discard_or_validate = match self.current_frame.discard {
            true => "DISCARD",
            false => "VALIDATE"
        };

        log::debug!(
            "{}: {} (0x{:02X}) @ [{:04X}:{:04X}]", 
            discard_or_validate, 
            name, 
            self.current_frame.opcode, 
            self.current_frame.regs[0].cs, 
            self.current_frame.regs[0].ip
        );

        if self.current_frame.discard {
            return;
        }

        // memset prefetch
        //self.current_frame.prefetch_addr.fill(0);


        let mut reg_buf: [u8; 28] = [0; 28];
        ArduinoValidator::regs_to_buf(&mut reg_buf, regs);

        self.cpu.load(&reg_buf).expect("validate() error: Load registers failed.");
        
        self.cpu.run(
            &self.current_frame.instr,
            &mut self.current_frame.emu_prefetch, 
            &mut self.current_frame.emu_ops, 
            &mut self.current_frame.cpu_prefetch, 
            &mut self.current_frame.cpu_ops
        ).expect("validate() error: Run failed");

        self.cpu.store().expect("Failed to store registers!");

        /*
        // Create scratchpad
        self.scratchpad.fill(0);
        
        // Load 'load' procedure into scratchpad
        for i in 0..self.load_code.len() {
            self.scratchpad[i] = self.load_code[i];
        }

        // Patch load procedure with current register values
        let r = self.current_frame.regs[0];

        self.write_u16_scratch(0x00, r.flags);
        
        self.write_u16_scratch(0x0B, r.bx);
        self.write_u16_scratch(0x0E, r.cx);
        self.write_u16_scratch(0x11, r.dx);

        self.write_u16_scratch(0x14, r.ss);
        self.write_u16_scratch(0x19, r.ds);
        self.write_u16_scratch(0x1E, r.es);
        self.write_u16_scratch(0x23, r.sp);
        self.write_u16_scratch(0x28, r.bp);
        self.write_u16_scratch(0x2D, r.si);
        self.write_u16_scratch(0x32, r.di);

        self.write_u16_scratch(0x37, r.ax);
        self.write_u16_scratch(0x3A, r.ip);
        self.write_u16_scratch(0x3C, r.cs);

        // JMP 0:2
        let jmp: Vec<u8> = vec![0xEA, 0x02, 0x00, 0x00, 0x00];
        for i in 0..jmp.len() {
            self.scratchpad[0xFFFF0 + i] = jmp[i];
        }

        self.code_as_data_skip = false;

        self.reset_sequence();

        self.state = ValidatorState::Setup;

        loop {
            self.execute_bus_cycle();
            self.next_bus_cycle();

            if self.state == ValidatorState::Finished {
                break;
            }
        }
        
        if self.code_as_data_skip {
            return
        }
        */

        /*
        for i in 0..NUM_MEM_OPS {
            //self.validate_mem_op(&self.current_frame.reads[i]);
            //self.validate_mem_op(&self.current_frame.writes[i]);
            self.validate_mem_op_read(i);
            self.validate_mem_op_write(i);
        }
        */

        self.validate_mem_ops();
        self.validate_registers();
    }

    fn emu_read_byte(&mut self, addr: u32, data: u8, read_type: ReadType) {
        if self.current_frame.discard {
            return;
        }

        match read_type { 
            ReadType::Code => {
                self.current_frame.emu_prefetch.push(
                    BusOp {
                        op_type: BusOpType::CodeRead,
                        addr,
                        data,
                        flags: MOF_EMULATOR
                    }
                );
                log::trace!("EMU fetch: [{:05X}] <- 0x{:02X}", addr, data);
            }
            ReadType::Data => {
                self.current_frame.emu_ops.push(
                    BusOp {
                        op_type: BusOpType::MemRead,
                        addr,
                        data,
                        flags: MOF_EMULATOR
                    }
                );
                log::trace!("EMU read: [{:05X}] <- 0x{:02X}", addr, data);
            }
        }

        
    }

    fn emu_write_byte(&mut self, addr: u32, data: u8) {
        
        self.visited[(addr & 0xFFFFF) as usize] = false;
        
        if self.current_frame.discard {
            return;
        }

        self.current_frame.emu_ops.push(
            BusOp {
                op_type: BusOpType::MemWrite,
                addr,
                data,
                flags: MOF_EMULATOR
            }
        );
        log::trace!("EMU write: [{:05X}] -> 0x{:02X}", addr, data);
    }

    fn discard_op(&mut self) {
        self.current_frame.discard = true;
    }
}
