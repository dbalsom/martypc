## /mount/floppy

This directory supports MartyPC's directory mounting feature. 

Directories here will be listed in the GUI for selection under the 'Load from Directory' menu within each drive submenu.
When a directory is selected, the files within the directory will be used to build a FAT12 image of the largest type 
supported by the relevant drive, unless a base image is specified (see below).

Currently, only the top level directories are in /mount/floppy are shown. 

An error will occur if the files present in the directory are too large to fit into the disk image. I decided this was 
more helpful than creating an image with fewer files than the user had anticipated, and perhaps not realizing why.

Currently, this feature is one-way only.  Changes to the disk image in memory cannot be saved back to the directory, 
although this is planned for the future. Eventually, I'd like to support saving the disk image in memory to an image 
file as well.

### Limitations

You cannot specify attributes for the created files, or specify the order in which they should be loaded to disk. This 
would otherwise make it difficult to create bootable disk images - but it is possible by providing a bootable base 
image. See the next section.

### Base Images

Instead of creating a new, formatted image, MartyPC can use a base image and add the files in the mount directory to it. 
Simply include a raw sector image named `baseimage.img` in the directory. This will also override the automatic image 
size determination. Be careful that the provided image is compatible with the currently configured drive size.

Files present in the base image will be considered 'read only' and will not be written back to the host for any reason.

An example base image that provides a FreeDOS-formatted, bootable diskette is provided in the `/mount/bootable` directory.

### Custom Bootsector

You can inject a custom bootsector into the resulting image by providing a `bootsector.bin` file of up to 512 bytes. 
If the file provided is less than that, it will be padded to 512 bytes and the boot marker added to the last two bytes 
(0x55AA). This makes it convenient to assemble small COM programs directly as boot sectors.

The boot sector will be loaded on top of any base image, if specified. 

Do note that DOS boot sectors have specific requirements, and reference disk geometry. You cannot take the boot sector
from a DOS-bootable 360K image and use it on a 720K image, or vice versa. Attempting to do so will result in garbled
directory listings and other corruption when interacting with the disk in DOS.










