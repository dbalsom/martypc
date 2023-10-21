
## [0.1.4](https://github.com/dbalsom/martypc/releases/tag/0.1.4) (2023-08-XX)

* CPU: Updated disassembler to normalize output against iced-x86. Now resolves negative immediates and displacements.
* CPU: Fixed typo for 'bp+di+DISP' in both disassemblers (Thanks Tom Harte)
* Added 8088 JSON CPU test generator and validator
* PIC: Honor IRQ offset specified in IWC2 to PIC (Thanks Folkert)
* BUS: Added MMIO peek functions. Allows Memory debug viewer to peek into MMIO regions, if device supports.
* CPU: Brand new, simplified BIU state logic
* CPU: Fixed & Improved DMA refresh scheduling. (Fixes 8088MPH CPU test)
* CPU: Fixed issue where Call Stack could grow uncontrollably with recursive code or interrupts
* CPU: Fixed CS:IP reporting in Instruction trace mode logs
* CPU: Fixed memory leak in Instruction trace mode (thanks Folkert)
* CPU: Fixed CPU cycle timings for LES and LDS instructions
* CPU: Fixed CPU issue where incorrect microcode jump was listed for fixed word displacements
* CPU: Fixed CPU issue where a prefetch abort would not properly override a prefetch delay
* CGA: Fully reset the CGA device on reboot. May(?) fix issue with black screens in 8088MPH. (Thanks hirudov)
* CGA: Added basic CGA snow emulation. Not yet 100% accurate.
* Fixed screenshot function when aspect-correction is off
* Fixed mouse capture hotkey (CTRL-F10)
* KEYBOARD: Add debug_keyboard config flag - this will print keyboard event info to the console for support
* CGA: Don't recalculate composite parameters if mode change was enable bit only

## [0.1.3](https://github.com/dbalsom/martypc/releases/tag/0.1.3) (2023-07-06)

* Disabled window doubling if window would not fit on screen due to DPI scaling.
* Decreased minimum window size
* Disabled warpspeed config flag
* Renamed 'autostart' config flag to 'auto_poweron' and fixed poweron logic.
* Mapped Right Alt, Control and Shift to emulated Left Alt, Control and Shift.
* Added UI warning when MartyPC is compiled in debug mode (unfortunately the default behavior of cargo build)
* Replaced CGA composite simulation code with reenigne's color multiplexer algorithm, for more accurate colors and a 3x speed improvement.
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
* Redesigned CGA card with 'dynamic clocking' support. Card will now switch between clocking by cycle or character as appropriate.
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
* Use fast_image_resize crate for SIMD acceleration. Aspect correction is now approximately 5X faster with equivalent quality.
* Fixed inaccuracy in keyboard shift register handling 
* Fixed bug in PIT latch logic (thanks 640KB)
* Fixed bug in PIC IRR logic (thanks 640KB)
* Fixed bug in PPI handling of keyboard enable line (Fixes halt on boot on 5160)
* Added CTRL-ALT-DEL menu option
* Known issues:
    * Turbo mode may cause the IBM BIOS to halt during POST during PIT checkout.
    * Formatting floppies is limited to 360K due to fixed drive type. 
    * Regression: PIT latch logic change has now made 8088MPH report a 1% CPU variation. I believe this is more a timer issue than a CPU issue.

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
