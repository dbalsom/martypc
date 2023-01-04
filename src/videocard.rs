/*

    videocard.rs

    Defines the VideoCard trait which any video card device (CGA, EGA, VGA) must implement.
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

use std::collections::HashMap;

//pub const TEXTMODE_MEM_ADDRESS: usize = 0xB8000;

use crate::config::VideoType;

pub type VideoCardState = HashMap<String, Vec<(String,String)>>;

/// All valid graphics modes for CGA, EGA and VGA Cards
#[allow (dead_code)] 
#[derive(Copy, Clone, Debug)]
pub enum DisplayMode {
    Disabled,
    Mode0TextBw40,
    Mode1TextCo40,
    Mode2TextBw80,
    Mode3TextCo80,
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
    pub addr: u32,
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


pub trait VideoCard {

    fn get_video_type(&self) -> VideoType;

    /// Returns the currently configured DisplayMode
    fn get_display_mode(&self) -> DisplayMode;

    fn get_display_extents(&self) -> (u32, u32);

    fn get_clock_divisor(&self) -> u32;

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

    /// Returns a vector with CRTC register name and value pairs
    //fn get_crtc_string_state(&self) -> Vec<(String, String)>;

    /// Returns a hash map of vectors containing name and value pairs.
    /// 
    /// This allows returning multiple categories of related registers.
    /// For the EGA for example, there are CRTC, Sequencer, Attribute and Graphics registers.
    fn get_videocard_string_state(&self) -> HashMap<String, Vec<(String,String)>>;

    /// Runs the video card device for the specified period of time
    fn run(&mut self, cpu_cycles: u32);

    /// Reset the video card
    fn reset(&mut self);

    /// Read pixel raw value
    fn get_pixel_raw(&self, x: u32, y:u32) -> u8;

    /// Read pixel color value as RGBA
    fn get_pixel(&self, x: u32, y: u32) -> &[u8];

    /// Return the specified bitplane as a slice
    fn get_plane_slice(&self, plane: usize) -> &[u8];

    /// Dump graphics memory to disk
    fn dump_mem(&self);

}