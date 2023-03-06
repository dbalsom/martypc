![pc_pixel2x](https://user-images.githubusercontent.com/7229541/176571877-ead7fb9c-0a83-41b1-8c51-ff8deeea7c5f.png)
# Marty 

## Introduction

Marty is a cross-platform IBM PC emulator written in Rust. It should build on Windows, Linux and MacOS (Including M1)

### Why another PC emulator?

This was a hobby project to see if I could write an emulator from scratch. This was also my first project learning Rust, so please be kind.

## Accuracy

Since Nov 2022, I have been working diligently to make Marty's 8088 CPU emulation cycle-accurate.  I have been validating the operation of the CPU against a real 8088 connected to an Arduino Mega. See my [Arduino8088 project](https://github.com/dbalsom/arduino_8088) for more details. 

The 8088 processor instruction queue is fully implemented. Bus delays, prefetch aborts, etc are all modelled so that instructions run in the proper time whether they are prefetched or not.

Currently, 8088mph reports my CPU is within 3% of expected. I believe that this is mostly due to missing DMA for DRAM refresh.

## Requirements

Marty requires an original IBM PC 5150 or 5160 BIOS ROM be placed in a /roms folder. I hope to support a free BIOS at some point which I can distribute or at least link to. In the meantime Google is your friend. For hard disk support you will also need the 20Mbit Fixed Disk Adapter ROM. 

Place floppy raw sector images (IMA or IMG) in a /floppy folder and Marty will find them on start-up. Floppy images up to 360k are supported.

## Features

Currently Marty will emulate an original IBM 5150 PC or 5160 XT if supplied the appropriate BIOS.

The following devices are at least partially implemented:
* CGA Card - Basic graphics and text modes are supported. I would like to rewrite this to do proper CRTC emulation.
* EGA/VGA Cards - Basic graphics modes are supported: 320x200, 640x350 & 640x480 16-color, and Mode13 (320x200 /w 256 colors)
* µPD764 Floppy Disk Controller - Enough FDC commands are implemented to make DOS happy.
* IBM 20MB Fixed Disk Controller - Emulated with basic VHD support, although only one specific drive geometry is supported so you will need to use the VHDs created by the emulator.
* 8255 PPI - Dip switches, speaker gate, keyboard input.
* 8259 PIC - Mostly implemented.
* 8253 PIT - Mostly implemented; currently lacking BCD mode.
* 8237 DMA Controller - Mostly implemented, but DMA transfers are currently "faked"
* 8250 UART - COM1 hard-coded to mouse, COM2 is available for serial passthrough to a host adapter.
* Mouse - A standard Microsoft Mouse is implemented on COM1.
* PC Speaker - Beeps and boops, although still a little glitchy. No speaker response modelling.

Marty has a GUI with a few useful debugging displays including the current instruction disassembly, memory, and various internal chip states. 



## Screenshots
![win30](https://user-images.githubusercontent.com/7229541/222996518-479e2c3a-40cd-4a69-b2fb-145a30219812.PNG)
![monkey_ega](https://user-images.githubusercontent.com/7229541/190879975-6ecba7c4-0529-4e34-ac6b-53827944e288.PNG)
![keen4](https://user-images.githubusercontent.com/7229541/182751737-85f2b9d1-d3b4-4b96-888c-3e8762c6c458.PNG)
![cat](https://user-images.githubusercontent.com/7229541/173169921-32b5dbad-0cb7-4cfa-921f-09ba7f946e85.png)

Composite CGA Simulation:

![kq1b](https://user-images.githubusercontent.com/7229541/175355050-af26243c-4a6e-49dd-9b01-991bc3420cb2.png)
