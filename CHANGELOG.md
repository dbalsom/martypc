
## [0.3.0](https://github.com/dbalsom/martypc/releases/tag/0.2.3) (2024-XX-XX)

### Directory & Zip Floppy Mounting

* Thanks to [rust-fatfs](https://github.com/rafalh/rust-fatfs), we can now dynamically build FAT12 images. This enables
  mounting both directories and ZIP archives as floppy images. Of course the contents must fit! By default, an image of 
  the largest supported size will be created. There are a few options as well to enable creation of bootable diskettes.
  See the Wiki for more information.

### Memory Visualizer

* Using the new Memory Visualizer window, you can now view the contents of memory graphically, interpreting raw 
  bytes as rendered text mode, or 1,2,4 or 8bpp pixels. This is a good way to explore the contents of memory, and when 
  investigating a running game one can find things like the game's back buffer as well as sprites loaded into memory.

### New Sound System

* MartyPC's original sound system only supported the PC speaker, and had a very awkward design (one might call it a 
  gross hack). For 0.3.0, MartyPC has a new sound system. The emulator core determines how many sound producing devices
  are installed and creates crossbeam channels for each device to send samples to the frontend, which is responsible for
  mixing and playback.

  The sound backend was changed from cpal to rodio to take advantage of the latter's sound mixing capabilities.


* Known Issues:
  * Some audio latency may be experienced with certain output devices, especially USB speakers that set a large default 
    buffer size.

### New Devices

* #### Adlib Music Card

  * The first audio device in MartyPC besides the PC speaker is the Adlib Music Card. OPL2 emulation is provided by the 
    highly-accurate [nuked-opl3](https://github.com/nukeykt/Nuked-OPL3) library. This marks the first device I have not 
    emulated myself, but the time it would take to research FM synthesis and produce anything near nuked-opl3 quality 
    would have you waiting for OPL2 emulation for another decade.

* #### VGA 

  * The IBM VGA card gets an initial implementation at last. MartyPC's VGA is based off its EGA implementation, with 
    appropriate changes and additions. Clocked up to 28Mhz, the VGA is an expensive device to run at a character-clock
    accurate rate, so you may need a fast computer. Aperture definitions may not be final. Mode 13h and ModeX/Y are 
    supported (Wolfenstein 3D's 'ModeY' works!) 
 
### Frontend Bug Fixes / Improvements
  * Performance Viewer:
    * Show stats for Audio sources 
  * File tree browser: 
    * Display directories before files
    * Display correct icons for different file types
  * IVT viewer: Display interrupt numbers as hex

### Core Bug Fixes / Improvements

* BUS: Fixed race condition in DMA scheduler update
* BUS: Reworked ByteQueue trait to support MMIO resolution
* TGA: Implement hi-res 2bpp mode (PCJr Colorpaint, etc)
* FDC: Refactoring of various functionality from FDC to FloppyDrive
* FDC: Fix PCjr keyboard watchdog timer handling
* BUS/PPI: Improve PCjr keyboard handling  
* BUS: New sound system. Removed cpal dependency from core. Core now produces samples and sends them to the frontend for 
  mixing and playback.
* PPI: Fixed memory bank DIP switch masks for memory configurations less than <64K.

### Debugger Bug Fixes / Improvements

* Disassembly Viewer: Now displays disassembly from MMIO regions.

### Distribution Changes

* Added new definition for an alternate 64K PCjr ROM dump (Thanks ImperatorBanana)
* Added new definition for an alternate 32K BASIC C1.0 ROM (thanks Torinde)

### Dependency Updates


## [0.2.2](https://github.com/dbalsom/martypc/releases/tag/0.2.2) (2024-06-22)

### New devices

* #### LoTech 2MB EMS Board
  * Added emulation of the [LoTech 2MB EMS Card](https://texelec.com/product/lo-tech-ems-2-mb/)
    This card can be added to any machine configuration via the `lotech_ems` overlay. You can specify the window segment
    and IO base address. However, these values must match one of the values supported by the real hardware or the driver 
    won't work with it. 

* #### Game Port and Joystick
  * Added emulation of the IBM game port card, and basic keyboard-based joystick emulation. There is a new keyboard 
    hotkey (`JoyToggle`) to turn this on and off (provisionally defined as Ctrl-F9), as well as configurable 
    `joystick_keys` in the configuration you can use to define what keys control the joystick.
  * PCJr and Tandy 1000 machines will have a game port installed automatically. You can add a game port to any PC or XT
    configuration via the `game_port` overlay.
  * Two two-button, two-axis joysticks are assumed to be connected when you specify a game port. 
    Different joystick configurations may be supported in the future. 

* #### PCJr Cartridge Slots
  * Added support in the core, frontend and GUI for PCJr cartridges in JRipCart format. Inserting or removing a cart
    will reboot the machine. You will only see the cartridge slots when using the PCJr machine.

### Frontend Bug Fixes / Improvements

* Added a new resource type 'cartridge' and menu interface to browse and select PCJr Cartridges.

### Core Bug Fixes / Improvements

* SERIAL: Fixed many issues in MartyPC's 8250 UART emulation. PCjr now boots without error code 'D' and Checkit2 serial
  diagnostics also pass.
* BUS: Implemented a `terminal_port` configuration option under `[machine]` in the main configuration. Writes to this 
  port will be printed to the host's terminal.
* MC6845: Fixed an issue preventing entering vertical total adjust period if vertical total was 127. Fixes some Hercules
  display issues.
* HERCULES: Increased the size of the Hercules' display field to accomodate some CGA emulators that drive the MDA
  monitor slightly out of sync (Fixes BBSIMCGA)
* CGA: Added CGA's external mode register to debug output
* MACHINE: Added a facility to record disassembly listings from running code. The output filename is set by 
  `disassembly_file` under `[machine]` in the main configuration.
  Basically, this feature saves instruction disassembly to a hash table by CS:IP.  Modification of code segments will 
  override previous disassembly, so it is most useful to toggle this feature on and off for specified periods.

### Debugger Bug Fixes / Improvements

* Serial Status window: Displays serial port registers and statistics.
* IO Status Window
  * Added a 'reset' button to reset all the port counters.
  * Added the last read byte value for each port
  * Fixed panic/crash when resetting machine with the IO Stats window open and scrolled. 

### Distribution Changes

* Added a SvarDOS-based MartyPC boot diskette to media/floppies/boot.
  This disk will load the LoTech EMS driver and CTmouse driver.
* Moved FreeDOS to media/floppies/DOS
* Added SvarDOS build 20240201 diskettes to media/floppies/DOS
* Added ctmouse v1.91 (last working version for Non-VGA) to media/floppies/utilties/mouse
* Added LoTech 2MB EMS card utilities to media/utilities/EMS
* Added JOYCALIB to media/utilities/joystick
* Updated GLaBIOS 0.2.6 ROMS for a bugfix when int 10h vector is overridden

### Dependency Updates

* Set rustc minimum version to 1.76
* Update egui to 0.27.2
* Update wgpu to 0.19.4

## [0.2.1](https://github.com/dbalsom/martypc/releases/tag/0.2.1) (2024-06-09)

### New BIU logic

* 0.2.0 introduced new BIU logic. Unfortunately, 0.2.0 took so long that this 'new' logic already needed 
  replacement. With the discovery of the 8088's 7-cycle bus access time, far simpler BIU logic is possible and has been 
  implemented in 0.2.1. For more information, see 
  [my corresponding blog post](https://martypc.blogspot.com/2024/02/the-complete-bus-logic-of-intel-8088.html).

* Along with this new logic comes a slightly different cycle log format; I'm probably the only person on earth that reads
  these logs, but it's worth mentioning. Several old state columns are gone, and now bus and T-cycles are displayed within 
  two separate 'pipeline slots'. 

### New CPU Framework

* MartyPC now has a framework to support multiple CPU types, so we can have CPUs other than the Intel 8088. 
  There is a slight performance hit involved in not hardcoding a specific CPU, but I believe there are some strategies
  I can use to mitigate this.

### New CPU Type

* An initial implementation of the NEC V20 has been added. All native-mode V20 instructions are implemented (except BRKEM).
  This is not cycle-accurate yet, as it was copied and modified from the 8088.
* The Arduino8088 project and the test generation engine of MartyPC were refactored and
  improved to enable generation of a CPU test suite for the V20, which will be used to incrementally improve V20 emulation
  accuracy over time. 8080 emulation mode will be added in the future.
  The V20 test suite can be found [here](https://github.com/singleStepTests/v20).
* You can add a V20 to any 8088-based machine by adding the `cpu_v20` overlay.

### New Machines

* Added Tandy 1000 and IBM PCJr machine types and ROM definitions. These are still very much a work in progress.
  * Tandy 1000 is mostly functional, and can accept a ibm_xebec hard disk controller (I will eventually dump the Tandy
    hard disk controller ROM, but it is a Xebec controller).
  * IBM PCJr needs cartridge browser support and PIO mode implemented for the floppy disk controller.
    * RAW cartridge images can be loaded as ROMs using the standard ROM definitions; however this is less than ideal.
      I will be adding support for standard PCJr ROM dump formats.

### New Video Types

* To go with these new systems, there is now a TGA graphics type. Again, a work in progress. 
  * Tandy BIOS identifies the TGA and will boot into 8x9 text mode. Display apertures were tweaked for TGA to support
    this "tall" mode.
  * TGA adapter operates very differently from other adapter types as it does not have its own VRAM. 
  * New core functionality was required to allow mapping of MMIO reads and writes back to system memory.
  * High-resolution 640x200 2bpp mode not yet implemented.

* PCJr graphics are implemented as a subtype of TGA adapter
* Initial support for Hercules graphics card as a subtype of MDA adapter. Still a bit glitchy.

### Frontend Bug Fixes / Improvements

* Added missing overlay for dual 360k floppy configuration (thanks DragomirPazura)
* Fixed loading of keyboard mapping files
* Added CPI (Cycles Per Instruction) to benchmark mode.

### Debugger Bug Fixes / Improvements

* Added cycle stopwatch UI to CPU Control window. The stopwatch allows you to set two 'breakpoints' and measure execution
  time between them.
* 'Step-over' logic overhauled. Step-over cycle timeout removed. A step over may not terminate, but this is in line with 
  other debuggers. On further feedback, dropping you off in a random location after timeout was not useful.
  * The expected return address will be displayed in the CPU Control window in the event a step over does not return.
* You can now step-over `rep` string operations. 
* Re-entrant instructions such as `rep` string operations and `hlt` will remain at top of disassembly until completed
  and will not flood the instruction history with repeated entries
* Instruction history now shows when hardware interrupts, traps and NMIs occur (in dark red)
* Instruction history now shows when jumps were taken (in dark green)
* New IO Statistics display
    * Shows you each IO port accessed, port description, and number of reads and writes.
* Overhauled PPI viewer (work in progress)
* Added tooltips to CPU Control buttons to clarify operations

### Core Bug Fixes / Improvements

* VALIDATOR: Improvements to CPU validation system to support initial generation of V20 tests.
* VALIDATOR: Improvements to test generation allow prefetched tests for 8088/V20
* 8080: Fixed panic due to microcode slice overrun (Thanks Vutshi)
* 8080: Implemented all undefined flags except for division
* 8088/V20: Corrected Trap delay after trap flag pop (Fixes Landmark Service Diagnostics)
* 8088: Refactored instruction decode to a table-based lookup, replaced custom flags with values from group decode ROM.
* 8088: Converted SegmentOverride enum to Option<Segment>, simplifying segment resolution logic in many places
* 8088: New 7-cycle bus access logic
* 8088: Refactored CycleText log format
* 8088: Added cyle stopwatch support
* 8088: Don't count individual REP iterations toward instruction counter
* 8088: Fixed call stack tracing (recursive calls will still cause overflow)
  * Reduced size of call stack history to avoid ridiculously tall windows
* 8088: Fixed cycle count reporting in Instruction History
* 8088: Added Interrupt, Trap and NMI history types
* EGA: Can now specify EGA dipswitch in machine configuration.
  * Added an `ibm_ega_on_cga` config overlay with the right dipswitch for running FantasyLand
* BUS: Require IoDevice trait implementations to provide port description strings
* BUS: Add new functionality for MMIO trait implementors to access main memory (Supports TGA)
* BUS: Add PCJr-specific keyboard logic to generate NMI on keypress
* PIT: Preserve latch value across mode changes (Fixes Tandy 1000 POST)
* PPI: More accurate PPI emulation
  * Tandy 1000 and IBM PCJr specific PPI details added
  * Control register implemented, and PPI group modes are now tracked
  * New state dump format for new frontend PPI Viewer
  * Implemented keyboard scancode serializer for PCJr (Still has some bugs)
* CGA: Fixed last-line CRTC flag logic. Fixes some severe glitches in Area 5150 I didn't notice because I have only been
       testing the first and last effect. (Thanks Sudo)

### Known Issues

* Area5150: Some minor glitches during the Elephant effect text scroll (left side character glitch) and the end credits
  have a periodic black scanline inserted. These were introduced with some new CRTC logic that is demonstrably more 
  correct in other scenarios, but clearly has a few bugs to iron out.

## [0.2.0](https://github.com/dbalsom/martypc/releases/tag/0.2.0b) (2024-04-01)

### New Features

* #### New Display Manager Framework
    * New graphics backend abstraction layer will eventually allow frontends built for other graphics backends such as
      SDL
    * New scaling modes: Choose from Fixed, Integer, Fit and Stretch scaling options
    * Display Apertures: Choose how much of the emulated display field you wish to see:
        * No overscan
        * 'Monitor-accurate' overscan
        * Full overscan
        * Full display field including hblank and vblank periods (for debugging)
    * Hardware aspect correction: Aspect correction is now performed in the shader for reduced CPU load
    * Multi-window support: Define additional output windows in the configuration file
        * Multiple windows can display the same video card, with different display options
        * Windows can be set to a fixed size or scale factor, or made completely resizable
        * Windows can be pinned 'always on top'
        * Windows can be toggled full-screen via CONTROL-ALT-ENTER hotkey
    * Shader support: A basic CRT shader is built into the new display scaler
        * Internal shader features:
            * Barrel distortion
            * Corner radius (rounded corners)
            * Monochrome phosphor simulation
            * Scanline emulation synchronized to emulated video resolution
        * Presets for the internal scaler can be defined in configuration and applied to different windows

* #### New ROM Definition Framework
    * ROM set definitions are no longer hardcoded. They will be read from ROM set definition TOML files in the ROM
      directory.
    * Add your own custom ROM sets or contribute missing ROM definitions to MartyPC!

* #### New Machine Configuration Framework
    * Define multiple Machine hardware configurations, name them, select them in the main config or via command line
      argument.
    * Configure the amount of conventional memory, the number and type of floppy drives, video cards, serial ports and
      more!

* #### New Resource Manager Framework
    * Paths are to 'resource ids' are now fully configurable, and facilities to build file trees are provided.
      MartyPC can create needed directories on startup. The Floppy and VHD browsers have been rewritten to take
      advantage of this new system, and so now you can organize your media directories into subdirectories for
      convenience.

* #### EGA Video Card
    * EGA is back! A character-clocked EGA implementation is here, although it may still be a bit rough around the
      edges.
      EGA will continue to be polished in upcoming releases.
    * Features:
        * Functional emulation of each of the 5 LSI chips on the EGA
        * Per-scanline Pel panning - effects like the wibble in Beverly Hills Cop work
        * Line compare register - See the status bar in Catacombs 3d!
        * CGA compatibility mode - Play Alleycat!
        * Software fonts - change your DOS font, or see a graphical mouse cursor in text mode (Norton Utilities 6.0)
    * Known issues:
        * Visual glitches with n0p's Windows 3.0 EGA driver patched for 8088
        * Some more obscure registers not properly emulated / investigated (SOM, etc.)
        * Aperture definitions / adjustments not final
        * Implementation may be slow in parts - more optimization needed (SIMD?)

* #### MDA Video Card
    * Not quite as a flashy as EGA, but the MDA card type is now also supported, and moreover, you can install an MDA
      alongside a CGA or EGA card for a dual head display.
    * 9th column rendering and underline attributes supported
    * Includes the framework for a LPT port, which will now be detected
    * Known issues:
        * Needs optimization - due to the 9-dot character clock making 64-bit aligned writes impossible, MDA is
          currently slower to emulate than EGA.
* #### New Keyboard System
    * MartyPC now performs low-level emulation of a Model F keyboard instead of directly translating OS input events to
      the core
        * Model M emulation to come
    * Configurable typematic rate and delay
    * International keyboard layouts are now supported via translation files.
        * Translation files support all keycode names defined by w3c: [https://w3c.github.io/uievents-code/#code-value-tables](https://w3c.github.io/uievents-code/#code-value-tables)
        * Translation files can define direct scancode mappings or full macros
        * Initial translation files include US, UK and IT layouts. More to come. Help appreciated!
    * Configurable hotkey support

### Debugger/GUI Improvements

* Reorganized Debug menu
* Improved appearance of CPU State display
* Instruction Trace: In Csv trace mode, instruction trace now has a scrolling table with optional columns
* Memory Viewer: Fixed scrolling issues, disassembly popup now uses fixed-width font
* IVT Viewer: Fixed scrolling, IVT entries now animate on change, added annotations
* Instruction History - fields now align with Disassembly View, and cycle counts have been moved to the right
* Memory Viewer will now show values for memory mapped regions
* Improved VHD creator - should no longer be confusing to use
* Text Mode Viewer - View ASCII contents of video memory, which you can select and copy to clipboard
* New themes courtesy of [egui-themer crate](https://github.com/grantshandy/egui-themer)
* New notification system courtesy of [egui-notify crate](https://github.com/ItsEthra/egui-notify).
    * Implemented success/error notifications for disk and file operations, screenshots, etc.

### Frontend Bug Fixes / Improvements

* Implemented configurable CPU halt behaviors
* Re-added CTRL-ALT-DEL menu option
* New benchmark mode (enable in martypc.toml, or use --benchmark-mode)
* Floppy and HDD browsers now support subdirectories
* Write protection can be toggled for floppy drives with configurable default
* Sound initialization is now optional
* Added 8088 JSON CPU test generator and validator
    * Used to create the first [comprehensive test suite for the Intel 8088](https://github.com/TomHarte/ProcessorTests/tree/main/8088)
* Added debug_keyboard config flag - this will print keyboard event info to the console for support

### Core Bug Fixes / Improvements

* CPU: Refactored general registers to union types (not any faster, but code is somewhat cleaner)
* CPU: Refactor PC from u32 to u16 to address segment wrapping issues, implement ip() in terms of PC
* CPU: Instruction counts properly increment even when instruction history is off
* CPU: Fixed device ticks after interrupts
* CPU: Improved Halt/Resume logic and cycle timings (Thanks Ken Shirriff)
* CPU: New sigrok cycle log format for viewing cycle logs in sigrok PulseView logic analyzer
* CPU: Updated disassembler to normalize output against iced-x86. Now resolves negative immediates and displacements
* CPU: Fixed typo for 'bp+di+DISP' in both disassemblers (Thanks Tom Harte)
* CPU: Brand new, simplified BIU state logic (which now needs to be rewritten, again...)
* CPU: Fixed & Improved DMA refresh scheduling. (Fixes 8088MPH CPU test)
* CPU: Fixed issue where Call Stack could grow uncontrollably with recursive code or interrupts
* CPU: Fixed CS:IP reporting in Instruction trace mode logs
* CPU: Fixed memory leak in Instruction trace mode (thanks Folkert)
* CPU: Fixed CPU cycle timings for LES and LDS instructions
* CPU: Fixed CPU issue where incorrect microcode jump was listed for fixed word displacements
* CPU: Fixed CPU issue where a prefetch abort would not properly override a prefetch delay
* PIC: Revised edge-triggered mode to lower INTR if last unmasked IR line goes low
* PIC: Ignore IMR during INTA pulse
* PIC: Handle multiple IRR bits being unmasked at the same time
* PIC: Honor IRQ offset specified in IWC2 to PIC (Thanks Folkert)
* PIT: Simulate counter write latency. A delay tick will be inserted if a write occurs too close to falling edge of
  clock
* PIT: Revised count register loading logic. Counting element uses internal reload value
* CGA: Properly model CRTC last-line flag when hcc < 2
* CGA: Preliminary CGA snow emulation. Not yet 100% accurate
* CGA: Properly disable cursor if cursor start > maximum scanline
* CGA: Reverted color palette entry for black from dark gray to true black
* CGA: Fully reset the CGA device on reboot. May(?) fix issue with black screens in 8088MPH. (Thanks hirudov)
* CGA: Don't recalculate composite parameters if mode change was enable bit only
* Xebec HDC: Proceed from Reset state to WaitngForCommand after a delay (Fixes Minix boot issue)
* Xebec HDC: Implemented missing Read Sector Buffer command (Fixes panic in IBM diagnostics)

### Major dependency updates:

* wgpu to 0.18
* egui to 0.24.2
* pixels to 0.13
* winit to 0.29.15

## [0.1.3](https://github.com/dbalsom/martypc/releases/tag/0.1.3) (2023-07-06)

* Disabled window doubling if window would not fit on screen due to DPI scaling.
* Decreased minimum window size
* Disabled warpspeed config flag
* Renamed 'autostart' config flag to 'auto_poweron' and fixed poweron logic.
* Mapped Right Alt, Control and Shift to emulated Left Alt, Control and Shift.
* Added UI warning when MartyPC is compiled in debug mode (unfortunately the default behavior of cargo build)
* Replaced CGA composite simulation code with reenigne's color multiplexer algorithm, for more accurate colors and a 3x
  speed improvement.
* Added contrast, phase and CGA type controls to composite adjustment window.
* Update Pixels to 0.13
* Update egui and egui-wgpu 0.22
* Update winit to 0.29*
    * Winit 0.29 fixes reported keyboard issues with non-US keyboard layouts unable to type certain keys.
    * Winit 0.29 fixes excessively high updates per second on some Linux builds
    * Enabled Wayland support
* Enable basic clipboard integration in debugger for copy/paste of breakpoints. This feature will be expanded.
* Fork egui-winit 0.22 to manually update winit dependency to 0.29.

## [0.1.2](https://github.com/dbalsom/martypc/releases/tag/0.1.2) (2023-06-29)

* Relicensed MartyPC under the MIT license.
* Redesigned CGA card with 'dynamic clocking' support. Card will now switch between clocking by cycle or character as
  appropriate.
* Improved hsync logic, screens in all graphics modes are now horizontally centered properly.
* Added 1.44MB floppy image definition. Somehow, these are readable(!?) (thanks xcloudplatform for discovering this)
* Fixed CGA palette handling bug. Fixes California Games CGAMORE mode. (thanks VileR)
* Added short tick delay between writing PIC IMR and raising any unmasked IRR bit to INTR. Fixes halts on warm boot.
* Improved performance when CPU is halted.
* Added menu options to save changes to loaded floppy image(s).
* Fixed CPU cycle tracelogging
* Added port mirrors for CGA (thanks th3bar0n)
* Fixed address wrapping for graphics modes (thanks th3bar0n)
* Fixed handling of mode enable flag in text mode (thanks VileR)
* Implemented better composite adjustment defaults (Matches colors in 8088mph better)
* Switched from cgmath to glam vector library. Approx 30% speedup in CGA composite simulation.
* Utilized bytemuck crate to write 32 bits at a time for CGA index->RGBA conversion, about 3x performance improvement
* Reorganized project structure. Refactored emulator core to Rust library and frontend components.
* Added Criterion for benchmarking components.
* Update Pixels library to 0.12.1
* Use fast_image_resize crate for SIMD acceleration. Aspect correction is now approximately 5X faster with equivalent
  quality.
* Fixed inaccuracy in keyboard shift register handling
* Fixed bug in PIT latch logic (thanks 640KB)
* Fixed bug in PIC IRR logic (thanks 640KB)
* Fixed bug in PPI handling of keyboard enable line (Fixes halt on boot on 5160)
* Added CTRL-ALT-DEL menu option
* Known issues:
    * Turbo mode may cause the IBM BIOS to halt during POST during PIT checkout.
    * Formatting floppies is limited to 360K due to fixed drive type.
    * Regression: PIT latch logic change has now made 8088MPH report a 1% CPU variation. I believe this is more a timer
      issue than a CPU issue.

## [0.1.1](https://github.com/dbalsom/martypc/releases/tag/0.1.1) (2023-05-31)

* Compiled for CGA only
* Fixed CGA cursor handling
* Rescan media folders when opening Media menu
* Added barebones documentation
* Added icon resource for Windows build
* Added ROM override feature
* Added HDD drive1 functionality
* Known issues
    * Floppy images are read-only.

## [0.1.0](https://github.com/dbalsom/martypc/releases/tag/0.1.0) (2023-05-29)

* Limited testing preview
