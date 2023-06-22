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

    videocard.rs

    Defines the VideoCard trait which any video card device (CGA, EGA, VGA) 
    must implement.
*/

/* 
    Video Modes 

    Mode T/G   Resolution Adapter Colors Address
    ---- ----  ---------- ------- ------ -------
      00 Text     320x200     CGA    16g    b800
                  320x350     EGA    16g    b800
                  360x400     VGA     16    b800
      01 Text     320x200     CGA     16    b800
                  320x350     EGA     16    b800
                  360x400     VGA     16    b800
      02 Text     640x200     CGA    16g    b800
                  640x350     EGA    16g    b800
                  720x400     VGA     16    b800
      03 Text     640x200     CGA     16    b800
                  640x350     EGA     16    b800
                  720x400     VGA     16    b800
      04  Gfx     320x200     CGA      4    b800
      05  Gfx     320x200     CGA     *4    b800 *alt CGA palette
      06  Gfx     640x200     CGA      2    b800

      0D  Gfx     320x200     EGA     16    a000
      0E  Gfx     640x200     EGA     16    a000

      10  Gfx     640x350     EGA    *16    a000 *256k EGA
      12  Gfx     640x480     VGA     16    a000
*/

use crate::bus::DeviceRunTimeUnit;

use crate::devices::cga::CGACard;
#[cfg(feature = "ega")]
use crate::devices::ega::EGACard;
#[cfg(feature = "vga")]
use crate::devices::vga::VGACard;

// This enum holds variants that hold the various implementors of the VideoCard trait.
// This is used for enum dispatch, to avoid overhead of dynamic dispatch when calling
// video card methods.
pub enum VideoCardDispatch {
    None,
    Cga(CGACard),
    #[cfg(feature = "ega")]
    Ega(EGACard),
    #[cfg(feature = "vga")]
    Vga(VGACard),
}

// This enum determines the rendering method of the given videocard device. 
// Direct mode means the video card draws to a double buffering scheme itself,
// Indirect mode means that the video renderer draws the device's VRAM. I think 
// eventually I will want to move all devices to direct rendering.
pub enum RenderMode {
    Direct,
    Indirect
}

use std::collections::HashMap;
use std::path::Path;

//pub const TEXTMODE_MEM_ADDRESS: usize = 0xB8000;

use crate::config::VideoType;

#[allow(dead_code)]
pub enum VideoCardStateEntry {
    Value8(u8),
    Value16(u16),
    String(String),
    Color(String, u8, u8, u8),
}

pub type VideoCardState = HashMap<String, Vec<(String, VideoCardStateEntry)>>;

/// All valid graphics modes for CGA, EGA and VGA Cards
#[allow (dead_code)] 
#[derive(Copy, Clone, Debug)]
pub enum DisplayMode {
    Disabled,
    Mode0TextBw40,
    Mode1TextCo40,
    Mode2TextBw80,
    Mode3TextCo80,
    ModeTextAndGraphicsHack,
    Mode4LowResGraphics,
    Mode5LowResAltPalette,
    Mode6HiResGraphics,
    Mode7LowResComposite,
    Mode8LowResTweaked,
    Mode9PCJrLowResGraphics,
    ModeAPCjrHiResGraphics,
    ModeBEGAInternal,
    ModeCEGAInternal,
    ModeDEGALowResGraphics,
    ModeEEGAMedResGraphics,
    ModeFMonoHiresGraphics,
    Mode10EGAHiResGraphics,
    Mode11VGAHiResMono,
    Mode12VGAHiResGraphics,
    Mode13VGALowRes256
}

pub struct CursorInfo {
    pub addr: usize,
    pub pos_x: u32,
    pub pos_y: u32,
    pub line_start: u8,
    pub line_end: u8,
    pub visible: bool
}

pub struct FontInfo {
    pub w: u32,
    pub h: u32,
    pub font_data: &'static [u8]
}

pub enum CGAPalette {
    Monochrome(CGAColor),
    MagentaCyanWhite(CGAColor),
    RedGreenYellow(CGAColor),
    RedCyanWhite(CGAColor) // "Hidden" CGA palette
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum CGAColor {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    White,
    BlackBright,
    BlueBright,
    GreenBright,
    CyanBright,
    RedBright,
    MagentaBright,
    Yellow,
    WhiteBright
}

#[derive (Copy, Clone)]
pub struct DisplayExtents {
    pub field_w: u32,       // The total width of the video field, including all clocks except the horizontal retrace period
    pub field_h: u32,       // The total height of the video field, including all clocks except the vertical retrace period
    pub aperture_w: u32,    // Width in pixels of the 'viewport' into the video field. 
    pub aperture_h: u32,    // Height in pixels of the 'viewport' into the video field.
    pub aperture_x: u32,    // X offset of aperture.
    pub aperture_y: u32,    // Y offset of aperture.
    pub visible_w: u32,     // The width in pixels of the visible display area
    pub visible_h: u32,     // The height in pixels of the visible display area
    pub overscan_l: u32,    // Size in pixels of the left overscan area
    pub overscan_r: u32,    // Size in pixels of the right overscan area
    pub overscan_t: u32,    // Size in pixels of the top overscan area
    pub overscan_b: u32,    // Size in pixels of the bottom overscan area
    pub row_stride: usize,  // Number of bytes in frame buffer to skip to reach next row
}

pub trait VideoCard {

    /// Returns the type of the adapter.
    fn get_video_type(&self) -> VideoType;

    /// Returns the rendering mode of the adapter.
    fn get_render_mode(&self) -> RenderMode;

    /// Returns the currently configured DisplayMode
    fn get_display_mode(&self) -> DisplayMode;

    /// Returns a slice of u8 representing video memory
    //fn get_vram(&self) -> &[u8];

    /// Return the size (width, height) of the last rendered frame.
    fn get_display_size(&self) -> (u32, u32);

    /// Return the DisplayExtents struct corresponding to the last rendered frame.
    fn get_display_extents(&self) -> &DisplayExtents;

    /// Return the DisplayExtents struct corresponding to the current back buffer.
    //fn get_back_buf_extents(&self) -> &DisplayExtents;    

    /// Return the visible resolution of the current video adapter's display field.
    /// For CGA, this will be a fixed value. For EGA & VGA it may vary.
    fn get_display_aperture(&self) -> (u32, u32);

    /// Return the 16 color CGA color index for the active overscan color.
    fn get_overscan_color(&self) -> u8;

    /// Return the u8 slice representing the front buffer of the device. (Direct rendering only)
    fn get_display_buf(&self) -> &[u8];

    /// Return the u8 slice representing the back buffer of the device. (Direct rendering only)
    /// This is used during debug modes when the cpu is paused/stepping so we can follow drawing
    /// progress.
    fn get_back_buf(&self) -> &[u8];

    fn get_clock_divisor(&self) -> u32;

    /// Get the position of the CRT beam (Direct rendering only)
    fn get_beam_pos(&self) -> Option<(u32, u32)>;

    /// Get the current scanline being rendered.
    fn get_scanline(&self) -> u32;

    /// Return a bool determining whether we double scanlines for this device (for CGA mostly)
    fn get_scanline_double(&self) -> bool;

    /// Get the current refresh rate from the adapter. Different adapters might
    /// support different refresh rates, even per mode.
    fn get_refresh_rate(&self) -> u32;

    /// Get the current calculated video start address from the CRTC
    fn get_start_address(&self) -> u16;

    /// Returns whether the current Display Mode has 40 col text
    fn is_40_columns(&self) -> bool;
    
    /// Returns whether the current Display Mode is a graphics mode
    fn is_graphics_mode(&self) -> bool;

    /// Returns a CursorInfo struct describing the current state of the text mode cursor.
    fn get_cursor_info(&self) -> CursorInfo;

    /// Return a FontInfo struct describing the currently selected font
    fn get_current_font(&self) -> FontInfo;

    /// Returns the currently programmed character height
    /// (CRTC Maximum Scanline + 1)
    fn get_character_height(&self) -> u8;

    /// Returns the current CGA-compatible palette and intensity attribute
    fn get_cga_palette(&self) -> (CGAPalette, bool);

    /// Returns a hash map of vectors containing name and value pairs.
    /// 
    /// This allows returning multiple categories of related registers.
    /// For the EGA for example, there are CRTC, Sequencer, Attribute and Graphics registers.
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String, VideoCardStateEntry)>>;

    /// Runs the video card device for the specified period of time
    fn run(&mut self, time: DeviceRunTimeUnit);

    /// Runs the video card for a the specified number of video clocks
    /// Used for debugging by advancing the video card independent of machine state.
    /// An implementor of VideoCard will have its own internal tick procedure.
    fn debug_tick(&mut self, ticks: u32);

    /// Reset the video card
    fn reset(&mut self);

    /// Read pixel raw value
    fn get_pixel_raw(&self, x: u32, y:u32) -> u8;

    /// Read pixel color value as RGBA
    fn get_pixel(&self, x: u32, y: u32) -> &[u8];

    /// Return the specified bitplane as a slice
    fn get_plane_slice(&self, plane: usize) -> &[u8];

    /// Return the number of frames the video device has rendered
    fn get_frame_count(&self) -> u64;

    /// Dump graphics memory to disk
    fn dump_mem(&self, path: &Path);

    /// Write a string to the video device's trace log (if one is configured)
    fn write_trace_log(&mut self, msg: String);

    /// Flush the trace log (if one is configured)
    fn trace_flush(&mut self);
}