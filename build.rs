/*
    MartyPC Emulator 
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

    --------------------------------------------------------------------------

    build.rs

    Build procedures for MartyPC.
*/

use {
    std::{
        env,
        io,
    },
    winres::WindowsResource,
};

fn main() -> io::Result<()> {
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("assets/martypc.ico")
            .compile()?;
    }
    Ok(())
}