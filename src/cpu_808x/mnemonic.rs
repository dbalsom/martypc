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

    ---------------------------------------------------------------------------

    cpu_808x::mnemonic.rs

    Defines mnemonic enum.

*/

#[allow(dead_code)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Mnemonic {
    InvalidOpcode,
    NoOpcode,
    NOP,
    AAA,
    AAD,
    AAM,
    AAS,
    ADC,
    ADD,
    AND,
    CALL,
    CALLF,
    CBW,
    CLC,
    CLD,
    CLI,
    CMC,
    CMP,
    CMPSB,
    CMPSW,
    CWD,
    DAA,
    DAS,
    DEC,
    DIV,
    ESC,
    FWAIT,
    HLT,
    IDIV,
    IMUL,
    IN,
    INC,
    INT,
    INT3,
    INTO,
    IRET,
    JB,
    JBE,
    JCXZ,
    JL,
    JLE,
    JMP,
    JMPF,
    JNB,
    JNBE,
    JNL,
    JNLE,
    JNO,
    JNP,
    JNS,
    JNZ,
    JO,
    JP,
    JS,
    JZ,
    LAHF,
    LDS,
    LEA,
    LES,
    LOCK,
    LODSB,
    LODSW,
    LOOP,
    LOOPNE,
    LOOPE,
    MOV,
    MOVSB,
    MOVSW,
    MUL,
    NEG,
    NOT,
    OR,
    OUT,
    POP,
    POPF,
    PUSH,
    PUSHF,
    RCL,
    RCR,
    REP,
    REPNE,
    REPE,
    RETF,
    RETN,
    ROL,
    ROR,
    SAHF,
    SALC,
    SAR,
    SBB,
    SCASB,
    SCASW,
    SETMO,
    SETMOC,
    SHL,
    SHR,
    STC,
    STD,
    STI,
    STOSB,
    STOSW,
    SUB,
    TEST,
    XCHG,
    XLAT,
    XOR,
}

impl Default for Mnemonic {
    fn default() -> Self { Mnemonic::InvalidOpcode }
}


