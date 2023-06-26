
## [0.1.2](https://github.com/dbalsom/martypc/releases/tag/0.1.2) (2023-06-XX)

* Redesigned CGA card with 'dynamic clocking' support. Card will now switch between clocking by cycle or character as appropriate.
* Fixed CGA palette handling bug. Fixes California Games CGAMORE mode. (thanks VileR)
* Added short tick delay between writing PIC IMR and raising any unmasked IRR bit to INTR. Fixes halts on warm boot.
* Improved performance when CPU is halted.
* Added menu options to save changes to loaded floppy image(s).
* Fixed CPU cycle tracelogging
* Added port mirrors for CGA (thanks th3bar0n)
* Fixed address wrapping for graphics modes (thanks th3bar0n)
* Fixed handling of mode enable flag in text mode (thanks VileR)
* Improved hsync logic for 40 column (7.15Mhz character clock) modes.
* Implemented better composite adjustment defaults (Matches colors in 8088mph better)
* Switched from cgmath to glam vector library. Approx 30% speedup in CGA composite simulation.
* Utilized bytemuck crate to write 32 bits at a time for CGA index->RGBA conversion, about 3x performance improvement
* Reorganized project structure. Refactored emulator core to Rust library and frontend components.
* Added Criterion for benchmarking components.
* Update Pixels library to 0.12.1
* Use fast_image_resize crate for SIMD accelleration. Aspect correction is now approx 5X faster with equivalent quality.
* Fixed innaccuracy in keyboard shift register handling 
* Fixed bug in PIT latch logic (thanks 640KB)
* Fixed bug in PIC IRR logic (thanks 640KB)
* Fixed bug in PPI handling of keyboard enable line (Fixes halt on boot on 5160)
* Added CTRL-ALT-DEL menu option

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
