
## [0.1.3](https://github.com/dbalsom/martypc/releases/tag/0.1.3) (2023-XX-xx)

* Disabled window doubling if window would not fit on screen due to DPI scaling.
* Decreased minimum window size
* Disabled warpspeed config flag.
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
