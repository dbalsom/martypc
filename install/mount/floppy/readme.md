## /mount/floppy

This directory supports MartyPC's directory to floppy image mounting feature. 

Directories here will be listed in the GUI for selection under the 'Load from Directory' menu within each floppy drive 
submenu. When a directory is selected, the files within the directory will be used to build a FAT12 image of the largest
type supported by the relevant drive.

Currently, only the top level directories are in /mount/floppy are shown. Any nested directories will be created in the
disk image.

An error will occur if the files present in the directory are too large to fit into the disk image. I decided this was 
more helpful than creating an image with fewer files than the user had anticipated, and perhaps not realizing why.

Currently, this feature is one-way only.  Changes to the disk image in memory cannot be saved back to the directory, 
although this is planned for the future. Eventually, I'd like to support saving the disk image in memory to an image 
file as well.

### Limitations

You cannot currently specify attributes for the created files, or specify the order in which they should be loaded to 
disk, except for DOS system files which will be detected and installed first (see below). 

### Custom Bootsector

You can inject a custom bootsector into the resulting image by providing a `bootsector.bin` file of up to 512 bytes. 
If the file provided is less than that, it will be padded to 512 bytes and the boot marker added to the last two bytes 
(0x55AA). This makes it convenient to assemble small COM programs directly as boot sectors.

The boot sector will be loaded on top of any base image, if specified. 

Note that DOS boot sectors reference specific disk geometry in the Bios Paramater Block (BPB). A boot sector from a 
DOS-bootable 360K image cannot be used directly in a 720K disk image without modification. MartyPC will attempt to 
detect the type of boot sector present, and if it can, will patch the Bios Parameter Block to support the resulting 
disk image size.

### Making a Bootable DOS Image

To make a bootable dos image from a directory, you will need a `bootsector.bin` containing a boot sector from a DOS 
diskette, as well as the system files for the appropriate version of DOS:

* For MS-DOS, include the files `IO.SYS`, `MSDOS.SYS` and `COMMAND.COM`
* For PC-DOS, include the files `IBMBIO.COM`, `IBMDOS.COM` and `COMMAND.COM`.
* For FreeDOS, include the file `KERNEL.SYS` and `COMMAND.COM`.

You may also wish to create an `AUTOEXEC.BAT` and `CONFIG.SYS` which will be handled as you would expect.












