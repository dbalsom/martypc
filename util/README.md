# The Marty Utilities

This directory contains several MS-DOS utilities that leverage MartyPC's internal service interrupt to perform specific functions.

All the included utilities should be built with the [Netwide Assembler (NASM)](https://www.nasm.us/)

## Building

The build requires Python 3 and NASM in `PATH`.

```text
python build_utils.py
```

The utilities are assembled as `.com` files and packaged in
`install/media/floppies/Utilities/MartyPC Utilities.zip`.

### `MDEBUG.COM`

This utility will launch the executable name given on the command line as a debugger using DOS `int 21h 4Bh` with `al==1`. 
It will then call an internal service interrupt to provide the emulator with the new processes CS:IP.
The emulator will then jump to this address and pause execution.

This provides a convenient way to debug an MS-DOS application using MartyPC's internal debugger.

#### Known Issues:

   - Larger applications may run out of memory

### `MPROBE.COM`

This utility reports whether MartyPC is running, returning the emulator version and service API version.

## `MQUIT.COM`

This utility instructs MartyPC to safely quit from inside the guest. It takes an optional parameter in seconds to delay
before quitting.

### `MSPEED.COM`

This utility immediately changes MartyPC's emulation speed. Pass an unsigned 16-bit decimal value in tenths of a percent;
for example, `MSPEED 1000` selects 100.0% (`1.0x`) and `MSPEED 500` selects 50.0% (`0.5x`). Values outside the configured
GUI speed range are clamped to that range. The utility queries the speed control before and after the request and prints
the current, requested, and final applied speeds. Use `MSPEED /S value` to set the current speed silently.

### `MRECV.COM`

This utility will receive a file from the host operating system, saving it to the specified filename. Use
`MRECV /N <destination>` for a non-interactive transfer, which automatically loads the requested file from the
`file_transfer` resource directory.

### `MSEND.COM`

This utility will send a file from DOS to the host operating system. Use `MSEND /N <file>` for a non-interactive transfer,
which writes the file to the `file_transfer` resource directory.


## Authoring new utilities

See the `SERVICE_API.md` document for documentation on MartyPC's internal service interrupts.
