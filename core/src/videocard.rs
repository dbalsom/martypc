/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

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

      07  Text    720x350     MDA      2    b800

      0D  Gfx     320x200     EGA     16    a000
      0E  Gfx     640x200     EGA     16    a000

      10  Gfx     640x350     EGA    *16    a000 *256k EGA
      12  Gfx     640x480     VGA     16    a000
*/

use std::{collections::HashMap, path::Path, str::FromStr};

use crate::bus::DeviceRunTimeUnit;

use crate::devices::cga::CGACard;
#[cfg(feature = "ega")]
use crate::devices::ega::EGACard;
#[cfg(feature = "vga")]
use crate::devices::vga::VGACard;

use serde::Deserialize;
use serde_derive::Serialize;

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum VideoType {
    None,
    MDA,
    CGA,
    EGA,
    VGA,
}

impl Default for VideoType {
    fn default() -> Self {
        VideoType::None
    }
}

impl FromStr for VideoType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "None" => Ok(VideoType::None),
            "MDA" => Ok(VideoType::MDA),
            "CGA" => Ok(VideoType::CGA),
            "EGA" => Ok(VideoType::EGA),
            "VGA" => Ok(VideoType::VGA),
            _ => Err("Bad value for videotype".to_string()),
        }
    }
}
#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum ClockingMode {
    Cycle,
    Character,
    Scanline,
    Dynamic,
}
impl Default for ClockingMode {
    fn default() -> Self {
        ClockingMode::Dynamic
    }
}

impl FromStr for ClockingMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        match s {
            "Cycle" => Ok(ClockingMode::Cycle),
            "Character" => Ok(ClockingMode::Character),
            "Scanline" => Ok(ClockingMode::Scanline),
            "Dynamic" => Ok(ClockingMode::Dynamic),
            _ => Err("Bad value for ClockingMode".to_string()),
        }
    }
}

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

// This struct provides an identifier for a VideoCard, encapuslating a unique numeric id ('idx')
// and the card's type. Hashable to store look
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct VideoCardId {
    pub idx:   usize,
    pub vtype: VideoType,
}

// This struct provides access to a VideoCard and its unique identifier.
pub struct VideoCardInterface<'a> {
    pub card: Box<&'a mut dyn VideoCard>,
    pub id:   VideoCardId,
}

// Video options that can be sent to a VideoCard device. Not all adapters will support
// each option. For example, CGA snow is of course only specific to the CGA card.
pub enum VideoOption {
    EnableSnow(bool),
}

// This enum determines the rendering method of the given videocard device.
// Direct mode means the video card draws to a double buffering scheme itself,
// Indirect mode means that the video renderer draws the device's VRAM. I think
// eventually I will want to move all devices to direct rendering.
pub enum RenderMode {
    Direct,
    Indirect,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub enum RenderBpp {
    #[default]
    Four,
    Six,
    Eight,
}

//pub const TEXTMODE_MEM_ADDRESS: usize = 0xB8000;

#[allow(dead_code)]
pub enum VideoCardStateEntry {
    Value8(u8),
    Value16(u16),
    String(String),
    Color(String, u8, u8, u8),
}

pub type VideoCardState = HashMap<String, Vec<(String, VideoCardStateEntry)>>;

/// All valid graphics modes for CGA, EGA and VGA Cards
#[allow(dead_code)]
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
    Mode13VGALowRes256,
}

pub struct CursorInfo {
    pub addr: usize,
    pub pos_x: u32,
    pub pos_y: u32,
    pub line_start: u8,
    pub line_end: u8,
    pub visible: bool,
}

pub struct FontInfo {
    pub w: u32,
    pub h: u32,
    pub font_data: &'static [u8],
}

pub enum CGAPalette {
    Monochrome(CGAColor),
    MagentaCyanWhite(CGAColor),
    RedGreenYellow(CGAColor),
    RedCyanWhite(CGAColor), // "Hidden" CGA palette
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
    WhiteBright,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum DisplayApertureType {
    #[default]
    Cropped,
    Accurate,
    Full,
    Debug,
}

#[derive(Copy, Clone, Debug)]
pub enum BufferSelect {
    Front,
    Back,
}

#[derive(Clone)]
pub struct DisplayApertureDesc {
    pub name: &'static str,
    pub aper_enum: DisplayApertureType,
}

/// A display aperture defines a visible subset rect of the total display field genereated by a
/// video card in Direct mode. w and h provide the dimensions of this rect, and x and y the
/// horizontal and vertical offsets from the origin (0,0)
/// Additionally, a debug flag is set to indicate whether an aperture should render debugging
/// information along with pixel data.
#[derive(Copy, Clone)]
pub struct DisplayAperture {
    pub w: u32,
    pub h: u32,
    pub x: u32,
    pub y: u32,
    pub debug: bool,
}

#[derive(Clone)]
pub struct DisplayExtents {
    pub apertures: Vec<DisplayAperture>, // List of display aperture definitions.
    pub field_w: u32,                    // The total width of the video field
    pub field_h: u32,                    // The total height of the video field
    pub row_stride: usize,               // Number of bytes in frame buffer to skip to reach next row
    pub double_scan: bool,               // Whether the display should be double-scanned when RGBA converted
    pub mode_byte: u8,                   // Mode byte. Used by CGA modes only.
}

pub trait VideoCard {
    /// Apply the specified VideoOption to the adapter.
    fn set_video_option(&mut self, opt: VideoOption);

    /// Returns the type of the adapter.
    fn get_video_type(&self) -> VideoType;

    /// Returns the rendering mode of the adapter.
    fn get_render_mode(&self) -> RenderMode;

    /// Returns the bit depth of the internal buffer for direct mode
    fn get_render_depth(&self) -> RenderBpp;

    /// Returns the currently configured DisplayMode
    fn get_display_mode(&self) -> DisplayMode;

    /// Override the clocking mode for the adapter.
    fn set_clocking_mode(&mut self, mode: ClockingMode);

    /// Returns a slice of u8 representing video memory
    //fn get_vram(&self) -> &[u8];

    /// Return the size (width, height) of the last rendered frame.
    fn get_display_size(&self) -> (u32, u32);

    /// Return the DisplayExtents struct corresponding to the last rendered frame.
    fn get_display_extents(&self) -> &DisplayExtents;

    /// Return a list of available display aperture names, indices, and the default aperture index
    fn list_display_apertures(&self) -> Vec<DisplayApertureDesc>;

    /// Return a list of display aperture definitions
    fn get_display_apertures(&self) -> Vec<DisplayAperture>;

    /// Return the 16 color CGA color index for the active overscan color.
    fn get_overscan_color(&self) -> u8;

    /// Return the u8 slice representing the selected buffer type. (Direct rendering only)
    fn get_buf(&self, buf_select: BufferSelect) -> &[u8];

    /// Return the u8 slice representing the front buffer of the device. (Direct rendering only)
    fn get_display_buf(&self) -> &[u8];

    fn get_clock_divisor(&self) -> u32;

    /// Return the status of VSYNC, HSYNC, and DISPLAY ENABLE, if applicable.
    fn get_sync(&self) -> (bool, bool, bool, bool);

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
    fn get_pixel_raw(&self, x: u32, y: u32) -> u8;

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
