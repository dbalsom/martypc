![pc_logo_with_text_v2_01](https://github.com/dbalsom/martypc/assets/7229541/ad0ed584-a7c7-40fe-aeb2-95e76051ae52)

# MartyPC

### User Guide

[Click here to access the MartyPC User Guide.](https://github.com/dbalsom/martypc/wiki/MartyPC-User-Guide)

MartyPC is a cross-platform IBM PC emulator written in Rust. Development began April, 2022. It should build on Windows, Linux and MacOS (Including M1). Currently, releases are Windows only.

### Why another PC emulator?

MartyPC was originally just a hobby project to see if I could write an emulator from scratch, while learning the Rust programming language. My original goals were modest, but MartyPC has reached a level of functionality that I never thought possible when originally starting. I would be happy if MartyPC could serve as a "reference emulator" - perhaps not the fastest or most fully featured, but written in a clear, readable way that describes the operation of the system and hardware, and is packed with debugging tools and ample logging for developers of emulators or oldschool software.

## Accuracy

I began work on making MartyPC's 8088 CPU emulation cycle-accurate in November of 2022. To do so, I validated the operation of the CPU against a real 8088 CPU connected to an Arduino MEGA microcontroller. See my [Arduino8088 project](https://github.com/dbalsom/arduino_8088) for more details. This allows an instruction to be simultaneously executed on the emulator and a real CPU and the execution results compared, cycle-by-cycle. More info on this process is described [on my blog](https://martypc.blogspot.com/2023/06/hardware-validating-emulator.html).

In April 2023, MartyPC became accurate enough to run the infamous PC demo, 8088MPH.

In May 2023, MartyPC became the first PC emulator capable of emulating every effect in the PC demo Area 5150. (See video here: https://www.youtube.com/watch?v=zADeLm9g0Zg )

In June of 2023, the WebAssembly build of MartyPC could run both [8088MPH](dbalsom.github.io/martypc/web/player.html?title=8088mph) and [Area 5150](dbalsom.github.io/martypc/web/player.html?title=area5150) in your web browser!

## Special Thanks

I have a long list of people to thank (See the About box!) but I would especially like to mention the contributions made by reenigne. Without his work reverse-engineering the 8088 microcode, this emulator would never have been possible. I also thank him for putting up with my endless questions.

## Features

Currently MartyPC will emulate an original IBM 5150 PC or 5160 XT.

The following devices are at least partially implemented:

* CGA Card - A fairly accurate, cycle-based implementation of the IBM CGA including the Motorola MC6845 CRTC controller allows MartyPC to run many demanding PC demos. Composite output and monitor simulation is supported, with MartyPC emulating an "old style" CGA.  Some work still remains on getting better composite color accuracy. 
* EGA/VGA Cards - Basic graphics modes are supported: 320x200, 640x350 & 640x480 16-color, and Mode13 (320x200 /w 256 colors). CGA compatibility modes remain unimplemented. May need conversion to cycle-accurate forms for games like Commander Keen. Work in progress. 
* µPD764 Floppy Disk Controller - Enough FDC commands are implemented to make DOS happy.
* IBM 20MB Fixed Disk Controller - Emulated with basic VHD support, although only one specific drive geometry is supported so you will need to use the VHDs created by the emulator.
* 8255 PPI
* 8259 PIC
* 8253 PIT - Recently rewritten after microcontroller-based research. At least one previously undocumented feature discovered. Accurate enough for PCM audio.
* 8237 DMA Controller - Mostly implemented, but DMA transfers are currently "faked". DRAM refresh DMA is simulated using a scheduling system.
* 8250 UART - COM1 hard-coded to mouse, COM2 is available for serial passthrough to a host adapter.
* Mouse - A standard Microsoft Mouse is implemented on COM1.
* PC Speaker - Beeps and boops, although still a little glitchy, it can produce reasonable PCM audio in demos such as 8088MPH, Area5150, and Magic Mushroom.

Marty has a GUI with a several useful debugging displays including instruction disassembly, CPU status, memory viewer, and various internal device states. 
![debugger01](https://github.com/dbalsom/martypc/assets/7229541/3eca1c16-470c-40ec-bb1a-6251677cf9ec)

## Screenshots

![area5150_title02](https://github.com/dbalsom/martypc/assets/7229541/373fff8b-2391-4ab3-a9a7-8062c496c78c)
![8088mph](https://user-images.githubusercontent.com/7229541/230502288-1d6f9d42-88b9-4e6c-8257-21378e68ff85.PNG)
![win30](https://user-images.githubusercontent.com/7229541/222996518-479e2c3a-40cd-4a69-b2fb-145a30219812.PNG)
![monkey_ega](https://user-images.githubusercontent.com/7229541/190879975-6ecba7c4-0529-4e34-ac6b-53827944e288.PNG)
![keen4](https://user-images.githubusercontent.com/7229541/182751737-85f2b9d1-d3b4-4b96-888c-3e8762c6c458.PNG)
![cat](https://user-images.githubusercontent.com/7229541/173169921-32b5dbad-0cb7-4cfa-921f-09ba7f946e85.png)
