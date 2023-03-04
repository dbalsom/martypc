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

pub enum BreakPointType {

    Execute(u16, u16), // Breakpoint on CS:IP
    ExecuteOffset(u16), // Breakpoint on *::IP
    ExecuteFlat(u32), // Breakpoint on CS<<4+IP
    MemAccess(u16, u16), // Breakpoint on memory access, seg::offset
    MemAccessFlat(u32), // Breakpoint on memory access, seg<<4+offset
    Interrupt(u8), // Breakpoint on interrupt #
}

