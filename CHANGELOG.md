
## [0.1.2](https://github.com/dbalsom/martypc/releases/tag/0.1.2) (2023-06-XX)

* Switched from cgmath to glam vector library. Approx 30% speedup in CGA composite simulation.
* Utilize bytemuck crate in CGA index->RGBA conversion, about 3x performance improvement
* Reorganized project structure. Using Criterion for benchmarking components.
* Update Pixels library to 0.12.1
* Use fast_image_resize crate for SIMD accelleration. Aspect correction is now approx 5X faster with equivalent quality.
* Fixed bug in PIT latch logic
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
    ** Floppy images are read-only.

## [0.1.0](https://github.com/dbalsom/martypc/releases/tag/0.1.0) (2023-05-29)

* Limited testing preview
