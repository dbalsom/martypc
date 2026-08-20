
# /crates

MartyPC is broken up into a lot of separate crates that can be used to compose an emulator front end.

The rationale is to be able to implement MartyPC on different backends and windowing systems (wint, eframe, wasm/web, SDL, etc.)

### /bin
 - This directory contains binary crates, such as MartyPC frontends or other utilities. You probably want to build one of these targets.

### /lib
 - This directory contains various library crates, including the main emulator core, `marty_core`.

