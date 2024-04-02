### MartyPC Machine Configuration

MartyPC uses "machine" configurations to describe the specific hardware to be emulated during a session.

Machine configurations can be split up among arbitrary TOML configuration files in this directory.

Each configuration consists of a base configuration definition and optional configuration overlay definitions.

A base configuration is capable of fully describing a machine. Overlays are supported in order to reduce the amount of
repeated configuration settings needed for common components, or to allow overriding certain settings within a
configuration.

An example configuration is commented below:

```toml
[[machine]]
name = "ibm5150_64k"    # Each configuration is required to have a name, which can be referenced 
                        # via the main configuration file or command line argument. 

type = "Ibm5150v64K"    # The Machine Type specifies the base hardware of this configuration. Think of this as
                        # describing the motherboard or fixed hardware configuration of a system. Here we
                        # are stating that this configuration builds on the base of an IBM 5150 with a 16-64K motherboard

rom_set = "auto"        # A specfic ROM set can be referenced by 'alias', or it can be left 'auto' to let MartyPC pick
                        # the best (usually newest) ROM set detected to be compatible for this system.

speaker = true          # Enable the PC speaker.          

[machine.memory]
conventional.size = 0xA0000     # List the amount of conventional memory. This is masked to the nearest multiple of
                                # 4k. Certain machine types may have more specific requirements. 
                                # For example, for the IBM 5150, this value should match a valid memory DIP setting.
                                # (See https://www.minuszerodegrees.net/5150/misc/5150_motherboard_switch_settings.htm)

conventional.wait_states = 0    # Wait states to apply to conventional memory (placeholder, not implemented)

# Floppy disk controller (optional)
[machine.fdc]
bus_type = "ISA"                # Bus type. Only supported type is ISA.
type = "IbmNec"                 # Type of floppy disk controller. Currently only "IbmNec" supported.

    # Drives connected to controller. Maximum of 4.
    [[machine.fdc.drive]]
    type = "360k"               # Drive type. Valid values are:
                                #  "360k"
                                #  "720k"
                                #  "1.2m"
                                #  "1.44m"
    image = "dos330.img"        # Default image to load into this drive. (optional) 
    
    [[machine.fdc.drive]]       # Additional floppy drive definitions follow
    type = "360k"

# Serial card (optional, repeatable)
[[machine.serial]]
bus_type = "ISA"                # Bus type. Only supported type is ISA.
type = "IbmAsync"               # Type of serial card. Currently only "IbmAsync" supported. This will add two
                                # serial ports to the system at 0x3F8 and 0x2F8.

    [[overlay.serial.port]]     # Eventually you will be able to specify individual port addresses and IRQs.
    io_base = 0x3F8             # For the moment these values are placeholders and are ignored.
    irq = 4
    [[overlay.serial.port]]
    io_base = 0x2F8
    irq = 3

# Video card (optional, repeatable)
[[machine.video]]
bus_type = "ISA"                # Bus type. Only supported type is ISA.
type = "MDA"                    # Type of video card. Valid values are:
                                #  MDA, CGA, EGA
clock_mode = "Default"          #  Clock mode for video card. Leave this "Default" in most cases.

# Keyboard (Optional)
[machine.keyboard]
type = "ModelF"                 # Type of keyboard installed. Currently only "ModelF" implemented. 
layout = "US"                   # Keyboard layout. Used to find a keyboard mapping file in /configs/keyboards/

# Serial mouse (Optional)
[machine.serialmouse]
type = "Microsoft"              # Type of serial mouse. Currently only "Microsoft" implemented.
port = 0                        # Serial port mouse is connected to. 
                                # Port 0 == first serial port defined (usually COM1)
                                # Port 1 == second serial port defined (usually COM2)

```

See the various TOML files provided for more examples.

### Machine Configuration Overlays

A machine configuration overlay can contain any part of a machine configuration that is (Optional). This includes
fdc, hdc, serial, video, mouse and keyboard sections.

If a base configuration and an overlay specify the same sections, the overlay will overwrite the base configuration's
values. If two overlays specify the same sections, they will be overwritten in the order the overlays were specified.

The following overlay definition can be used to add a CGA and MDA card to any base configuration:

```toml
[[overlay]]
name = "cga_and_mda"

    [[overlay.video]]
    bus_type = "ISA"
    type = "CGA"
    clock_mode = "Default"
    
    [[overlay.video]]
    bus_type = "ISA"
    type = "MDA"
    clock_mode = "Default"
```