![pc_pixel2x](https://user-images.githubusercontent.com/7229541/176571877-ead7fb9c-0a83-41b1-8c51-ff8deeea7c5f.png)
# Marty 

## Introduction

Marty is a cross-platform IBM PC emulator written in Rust. It should build on Windows, Linux and MacOS (Including M1)

### Why another PC emulator?

This was a hobby project to see if I could write an emulator from scratch. This was also my first project learning Rust, so please be kind.
I do not claim to be an expert in either emulators or Rust. 

This emulator is nowhere near cycle-accurate. It will not run the 8088mph demo anytime soon, but I do hope to improve accuracy over time.

## Requirements

Marty requires an original IBM PC 5150 or 5160 BIOS ROM be placed in a /roms folder. I hope to support a free BIOS at some point which I can distribute or at least link to. In the meantime Google is your friend. For hard disk support you will also need the 20Mbit Fixed Disk Adapter ROM. 

Place floppy raw sector images (IMA or IMG) in a /floppy folder and Marty will find them on start-up. Floppy images up to 360k are supported.

## Features

Currently Marty will emulate an original IBM 5150 PC or 5160 XT if supplied the appropriate BIOS.

The following devices are at least partially implemented:
* CGA Card - Basic graphics and text modes are supported. I would like to rewrite this to do proper CRTC emulation.
* µPD764 Floppy Disk Controller - Enough FDC commands are implemented to make DOS happy.
* IBM 20MB Fixed Disk Controller - Emulated with basic VHD support, although only one specific drive geometry is supported so you will need to use the VHDs created by the emulator.
* 8255 PPI - Dip switches & keyboard input.
* 8259 PIC - Mostly fully implemented.
* 8253 PIT - Enough modes to run basic games. Not all modes are implemented, neither is BCD mode.
* 8237 DMA Controller - Mostly fully implemented, but DMA transfers are currently "faked"
* 8250 UART - COM1 hard-coded to mouse, COM2 is available for serial passthrough to a host adapter.
* Mouse - A standard Microsoft Mouse is implemented on COM1.

Marty has a GUI with a few useful debugging displays including the current instruction disassembly, memory, and various internal chip states. 

## Missing features: (Planned)

* Better Configuration system
* Better debugger and breakpoint system

## Known Issues

* Windows 1.0 runs but PAINT.EXE crashes the system
* Magic Mushroom demo exits immediately
* 8088mph and area5150 demos don't run at all

## Wishlist features:

* EGA/VGA graphics

## Probably never implementing:

* SVGA
* Soundblaster/Adlib sound
* 80286+ processors

## Screenshots
![tools](https://user-images.githubusercontent.com/7229541/173169915-58b0bb5f-663c-41de-be3c-66952297558e.png)
![keen4](https://user-images.githubusercontent.com/7229541/182751737-85f2b9d1-d3b4-4b96-888c-3e8762c6c458.PNG)
![cat](https://user-images.githubusercontent.com/7229541/173169921-32b5dbad-0cb7-4cfa-921f-09ba7f946e85.png)

Composite CGA Simulation:

![kq1b](https://user-images.githubusercontent.com/7229541/175355050-af26243c-4a6e-49dd-9b01-991bc3420cb2.png)
