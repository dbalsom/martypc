
This README accompanies the ROM 'BOCHS-VGABIOS.bin'


Bochs VGABios
-------------

The goal of this project is to have a LGPL'd Video Bios in Bochs and QEMU.
This VGA Bios is very specific to the emulated VGA card.
It is NOT meant to drive a physical vga card.


Bochs VBE extension
---------------------

The Bochs VBE extension and it's Bochs device backend has been written by
Jeroen Janssen (japj). It implements support for VBE version 2.0.

Cirrus SVGA extension
---------------------

The Cirrus SVGA extension is designed for the Cirrus emulation in Bochs and
qemu. The initial patch for the Cirrus extension has been written by Makoto
Suzuki (suzu).


Voodoo Banshee PCI extension
----------------------------

The Voodoo Banshee PCI extension is designed for the Banshee emulation in Bochs.
Some parts of the initial version are based on the Cirrus extension code.


Install
-------
To compile the VGA Bios you will need the following packages:
- make
- gcc (for 'biossums', 'vbetables-gen' and VGABIOS preprocessing)
- dev86 (bcc, as86)

Untar the archive, and type 'make'. You should get this set of binary files:
"VGABIOS-lgpl-latest.bin", "VGABIOS-lgpl-latest-debug.bin",
"VGABIOS-lgpl-latest-cirrus.bin", "VGABIOS-lgpl-latest-cirrus-debug.bin",
"VGABIOS-lgpl-latest-banshee.bin" and "VGABIOS-lgpl-latest-banshee-debug.bin".
Alternatively, you can use one of the precompiled binary files present in
the archive.

Edit your bochs config file, and modify the 'vgaromimage' directive to point
it to the VGABIOS image you want to use.


Debugging
---------
You can get a very basic debugging system: the VGABIOS sends messages to a
usually unused ISA i/o port. The emulator prints the received characters to
log file or console. In Bochs the "unmapped" device plugin must be loaded.
It registers the VGABIOS info port 0x500.

VGABIOS images compiled with the DEBUG symbol set, will use the "printf"
function to write the messages to the info port.


Testing
-------
Look at the "testvga.c" file in the archive. This is a minimal Turbo C 2.0
source file that calls a few int10 functions. Feel free to modify it to suit
your needs.


Copyright and License
---------------------
The original version of this program has been written by Christophe Bothamy.
It is protected by the GNU Lesser Public License, which you should
have received a copy of along with this package.


Reverse Engineering
-------------------
The VGA Bios has been written without reverse-engineering any existing Bios.


Acknowledgment
--------------
The source code contains code ripped from rombios.c of plex86, written
by Kevin Lawton <kevin2001@yahoo.com>

The source code contains fonts from fntcol16.zip (c) by Joseph Gil avalable at :
ftp://ftp.simtel.net/pub/simtelnet/msdos/screen/fntcol16.zip
These fonts are public domain

The source code is based on information taken from :
- Kevin Lawton's vga card emulation for bochs/plex86
- Ralf Brown's interrupts list avalaible at
  http://www.cs.cmu.edu/afs/cs/user/ralf/pub/WWW/files.html
- Finn Thogersons' VGADOC4b available at http://home.worldonline.dk/~finth/
- Michael Abrash's Graphics Programming Black Book
- Francois Gervais' book "programmation des cartes graphiques cga-ega-vga"
  edited by sybex
- DOSEMU 1.0.1 source code for several tables values and formulas


Feedback
--------
Please report any bugs, comments, patches for this VGA Bios
on Github at: https://github.com/bochs-emu/VGABIOS
You can find the latest release at https://github.com/bochs-emu/VGABIOS/releases
For any information on this VGABIOS, see https://www.nongnu.org/vgabios/
For any information on bochs, visit the website https://bochs.sourceforge.io/
For any information on Qemu, visit the website http://wiki.qemu.org/
