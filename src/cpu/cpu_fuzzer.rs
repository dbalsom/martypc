
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
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::cpu::*;

const RNG_SEED: u64 = 0x58158258u64;

impl<'a> Cpu<'a> {

    pub fn randomize_seed(&mut self, mut seed: u64) {
        if seed == 0 {
            seed = RNG_SEED;
        }
        self.rng = Some(rand::rngs::StdRng::seed_from_u64(1234));
    }

    pub fn randomize_regs(&mut self) {

        self.cs = self.rng.as_mut().unwrap().gen();
        self.ip = self.rng.as_mut().unwrap().gen();

        self.reset(self.cs, self.ip);

        for i in 0..REGISTER16_LUT.len() {
            let n: u16 = self.rng.as_mut().unwrap().gen();
            self.set_register16(REGISTER16_LUT[i], n);
        }

        // Adjust pc
        self.pc = Cpu::calc_linear_address(self.cs, self.ip);
        // Flush queue
        self.queue.flush();

        self.ds = self.rng.as_mut().unwrap().gen();
        self.ss = self.rng.as_mut().unwrap().gen();
        self.es = self.rng.as_mut().unwrap().gen();


    }

    pub fn randomize_mem(&mut self) {

        for i in 0..self.bus.size() {

            let n: u8 = self.rng.as_mut().unwrap().gen();
            self.bus.write_u8(i, n).expect("Mem err");
        }
    }

}