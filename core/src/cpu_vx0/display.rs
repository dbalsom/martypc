/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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

    ---------------------------------------------------------------------------

    cpu_vx0::display.rs

    Formatting routines for mnemonics and Instruction type.
    Converts Instructions into string or token representations.

*/
/*
#[cfg(test)]
mod tests {
    #[cfg(feature = "cpu_validator")]
    use crate::cpu_validator;

    use crate::{cpu_vx0::*, syntax_token::*};

    #[test]
    fn test_display_methods_match() {
        let test_ct = 1_000_000;

        #[cfg(feature = "cpu_validator")]
        use cpu_validator::ValidatorMode;

        let mut cpu = Cpu::new(
            CpuType::NecV20,
            TraceMode::None,
            TraceLogger::None,
            #[cfg(feature = "cpu_validator")]
            ValidatorType::None,
            #[cfg(feature = "cpu_validator")]
            TraceLogger::None,
            #[cfg(feature = "cpu_validator")]
            ValidatorMode::Instruction,
            #[cfg(feature = "cpu_validator")]
            1_000_000,
        );

        cpu.randomize_seed(1234);
        cpu.randomize_mem();

        for i in 0..test_ct {
            cpu.reset();
            cpu.randomize_regs();

            if cpu.get_register16(Register16::IP) > 0xFFF0 {
                // Avoid IP wrapping issues for now
                continue;
            }
            let opcodes: Vec<u8> = (0u8..=255u8).collect();

            let mut instruction_address =
                Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));

            while (cpu.get_register16(Register16::IP) > 0xFFF0) || ((instruction_address & 0xFFFFF) > 0xFFFF0) {
                // Avoid IP wrapping issues for now
                cpu.randomize_regs();
                instruction_address =
                    Cpu::calc_linear_address(cpu.get_register16(Register16::CS), cpu.get_register16(Register16::IP));
            }

            cpu.random_inst_from_opcodes(&opcodes);

            cpu.bus_mut().seek(instruction_address as usize);
            let (opcode, _cost) = cpu.bus_mut().read_u8(instruction_address as usize, 0).expect("mem err");

            let mut i = match Cpu::decode(cpu.bus_mut()) {
                Ok(i) => i,
                Err(_) => {
                    log::error!("Instruction decode error, skipping...");
                    continue;
                }
            };

            let s1 = i.to_string();
            let s2 = SyntaxTokenVec(i.tokenize()).to_string();

            if s1.to_lowercase() == s2.to_lowercase() {
                //log::debug!("Disassembly matches: {}, {}", s1, s2);
            }
            else {
                println!("Test: {} Disassembly mismatch: {}, {}", i, s1, s2);
                assert_eq!(s1, s2);
            }
        }
    }
}
*/
