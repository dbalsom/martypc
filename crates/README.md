
# /crates

MartyPC is broken up into a lot of separate crates that can be used to compose an emulator front end.

The rationale is to be able to implement MartyPC on different backends and windowing systems (wint, eframe, wasm/web, SDL, etc).

### /binaries
 - This directory contains binary crates, such as MartyPC frontends or other utilities. You probably want to build one of these targets.

### /lib
 - This directory contains various library crates that implement functionality shared between frontend implementations.

### /marty_common
 - This crate defines types and interfaces that are common between frontends and MartyPC's core.

### /marty_core
 - This crate contains the main emulator core functionality. It is intended to be independent of windowing system, 
   rendering backend, sound backend, or other implementation-specific details.
