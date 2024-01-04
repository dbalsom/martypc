# MartyPC ROM and ROM Definition Guide

## ROM Directory

If you're reading this, you've found the main ROM directory! Place your ROMs in this directory. If this is the default
ROM directory (`/media/roms` in a portable install) then this entire directory will be searched recursively for ROM
definition files, and subsequently any defined ROM files.

Most common sources of ROM images for the IBM PC 5150 or IBM XT 5160 should work, including MAME, PCem, 86Box and minuszerodegrees.

If multiple valid sets of ROMs are detected for a specified machine, MartyPC will use the 'best' set as decided by OEM flag and 
release date.

Please see the MartyPC Wiki for more information on using ROMs with MartyPC:

https://github.com/dbalsom/martypc/wiki/ROMs

### Tips
 * If you're not sure what ROMs to use, you can copy all the ROMs from the PCem or 86Box ROM distributions into the ROMs folder, and
   MartyPC should find what it needs and ignore the rest.
 * You can run MartyPC with the `--romscan` argument to see what ROMs MartyPC finds and detects. If you're having issues with ROM detection, including
  this argument in your issue report would be very helpful.
 * If you wish to omit certain directories from search, such as 'parking', you can add those directory names to the main
configuration's 'ignore_dirs'. This can be handy to control which ROMs are *not* used if you're otherwise struggling with the ROM priority logic.

## ROM Definition Files
MartyPC uses **ROM Definition Files** to define and identify ROMs to use. This provides a flexible and extensible system.
If you find new ROMs that aren't defined yet, you can add them (although please let me know about them!)

If you are a ROM developer, you can create your own ROM definition files, and reference them in custom machine configurations.





