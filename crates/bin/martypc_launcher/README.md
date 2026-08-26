# martypc_laucher

This is a launchr front-end for MartyPC based on [eframe](https://github.com/emilk/egui/tree/master/crates/eframe).

Many people can struggle with setting up MartyPC and knowing how to change configurations. 

The current system of machine configurations and machine configuration overlays is powerful and flexible, but 
difficult to understand and not very well documented (and always in flux).

Obtaining ROMS (and the correct ones) is not always straightorward either, as is knowing where to put them.

The idea of the MartyPC Launcher is to scan the MartyPC distribution directory, enumerate machine configurations,
configuration overlays, and scan a list of roms, then allow a user to configure an available machine with the 
options they want. 

