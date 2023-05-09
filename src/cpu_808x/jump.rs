use crate::cpu_808x::*;
use crate::cpu_808x::biu::*;
use crate::cpu_808x::addressing::*;

impl<'a> Cpu<'a> {

    /// Execute the RELJMP microcode routine, optionally including the jump into the procedure.
    pub fn reljmp(&mut self, new_ip: u16, jump: bool) {
        if jump {
            self.cycle_i(MC_JUMP);
        }
        self.biu_suspend_fetch_i(0x0d2);
        self.cycles_i(3, &[0x0d2, 0x0d3, 0x0d4]);
        self.ip = new_ip;
        self.biu_queue_flush(); // 0d5
        self.cycle_i(0x0d5);
    }

    /// Execute the FARRET microcode routine, including the jump into the procedure.
    pub fn farret(&mut self, far: bool) {

        self.cycle_i(MC_JUMP);
        self.set_mc_pc(0x0c2);
        self.pop_register16(Register16::IP, ReadWriteFlag::Normal);
        self.biu_suspend_fetch();
        self.cycles_i(2, &[0x0c3, 0x0c4]);

        let far2 = self.i.opcode & 0x08 != 0;
        assert_eq!(far, far2);

        if far {
            self.cycle_i(MC_JUMP);
            self.pop_register16(Register16::CS, ReadWriteFlag::Normal);
            
            self.biu_queue_flush();
            self.cycles_i(2, &[0x0c7, MC_RTN]);
        }
        else {
            self.biu_queue_flush();
            self.cycles_i(2, &[0x0c5, MC_RTN]);
        }

        self.cycles_i(4, &[0x0c7, MC_RTN, 0x0ce, 0x0cf]);
    }
}