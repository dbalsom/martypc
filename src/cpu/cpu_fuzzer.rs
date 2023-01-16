
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
use rand::Rng;

use crate::cpu::*;

impl<'a> Cpu<'a> {

    pub fn randomize_regs(&mut self) {
        
        self.reset();
        let mut rng = rand::thread_rng();

        for i in 0..REGISTER16_LUT.len() {
            let n: u16 = rng.gen();
            self.set_register16(REGISTER16_LUT[i], n);
        }

        self.cs = rng.gen();
        self.ip = rng.gen();

        // Adjust pc
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
        // Flush queue
        self.queue.flush();

        self.ds = rng.gen();
        self.ss = rng.gen();
        self.es = rng.gen();


    }

    pub fn randomize_mem(&mut self) {

        let mut rng = rand::thread_rng();

        for i in 0..self.bus.size() {

            let n: u8 = rng.gen();
            self.bus.write_u8(i, n).expect("Mem err");
        }
    }

}