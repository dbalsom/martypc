### Floppy Directory

Put floppy images in this directory. Valid image formats are flat sector images
of the type created by WinImage, dd or other such utilities, typically with \*.img
or \*.ima extensions. Compressed \*.imz images are not supported.

MartyPC will adjust the floppy drive size within the capabilities of the currently
emulated machine, based on the size of the image loaded. Thus if you load a 720KB 
floppy image, the drive becomes a 720KB floppy drive. There is no need to configure
this setting.

However, if emulating a IBM PC or XT, note that certain floppy sizes will not work
as the machine/BIOS does not know how to handle a disk of that size.

Floppy images must be one of these exact file sizes:

   163,840 - 160KB floppy, single-sided  Rare, only used by the earliest PCs.
   184,320 - 180KB floppy, single-sided. Rare, only used by the earliest PCs.
   327,680 - 320KB floppy, double-sided. Somewhat rare. 
   368,640 - 360KB floppy, double-sided. Extremely common.
   737,280 - 720KB floppy, double-sided. Extremely common.
   1,228,800 - 1.2MB floppy, double-sided. Extremely common.
   
   The exception is anything smaller than 163,840 bytes will be loaded as that size,
   padded with 0s.
   This is a convenience feature for development of boot sector software or loading
   of boot sector demos and games.

