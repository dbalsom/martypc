The MartyPC Desktop frontend can be built targeting Windows, Linux, and MacOS

It may be possible to build on other platforms; please consider contributing instructions if you do.

## All Targets

The first step to building MartyPC is installing Rust on your system. Visit [the installation instructions at rust-lang.org](https://www.rust-lang.org/tools/install) and follow the instructions for installing Rust for your platform.

Next, make sure you have git installed. If you need to install it, you can use your system's package manager, or [install Git directly.](https://git-scm.com/downloads)

## Building for Windows

* Make sure you have installed the [MSVC build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
    * You don't need to install the entire Visual Studio community edition, but you can as an alternative to installing build tools.
    * When installing the build tools, you can select the C++ build option to reduce the amount of disk space you need.
* Open a command prompt
* `cd` to an appropriate directory (git will make a new subdirectory here when you clone the MartyPC repo)
* Clone the repository
    * Type `git clone https://github.com/dbalsom/martypc.git`
* Type `cd martypc/install`
* Make sure LIBCLANG_PATH is set 
    * e.g.) Type `set LIBCLANG_PATH=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\Llvm\x64\bin`
* Type `cargo run -r --features ega` (features may vary - see Cargo.toml for a list)

### Building for Linux

Building for Linux follows the same basic process, but you must have several development dependencies installed.
How you install them depends on your particular distribution.

#### Building on Ubuntu 23

* Assuming a minimal install:

  * Install git
    * `sudo apt install git`
  * Install curl
    * `sudo apt install curl`
  * Install build tools
    * `sudo apt build-essential`
  * Install Rust by running the command shown on the install page linked above. As of this writing, it is:
    * `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
    * Proceed with defaults to install Rust in your home directory
  * Open a new shell to set new environment variables
  * Install the following development dependencies with `sudo apt install [packagename]`
    * `pkg-config`
    * `libasound2-dev`
    * `libudev-dev`
  * `cd` to an appropriate directory 
  * Clone the martypc repo
    * `git clone https://github.com/dbalsom/martypc.git`
  * `cd martypc/install`
  * Build MartyPC!
    * `cargo run -r`
  

### Building for MacOS
* Open a Terminal window
* `cd` to an appropriate directory
* clone the repository
    * `git clone https://github.com/dbalsom/martypc.git`
* `cd` into the /install directory in the newly cloned repository
* type `cargo run -r --features ega` (features may vary - see Cargo.toml for a list)

### Contributing to MartyPC
* IDEs
    * If you'd like to use a IDE, I recommend [RustRover](https://www.jetbrains.com/rust/), which is currently free during EAP. I currently maintain build configurations for RustRover.
    * You can also use Visual Studio Code, but I haven't been updating the build configurations in a while, so you may need to update things. Please send a pull request for anything you have to fix.
* Pull Requests
    * PRs are welcome! Just be sure that you describe them well and have thoroughly tested them. Please open an issue or discussion before developing any major new functionality or feature.
    * Please respect the configured rustfmt settings - I recommend configuring your editor or IDE to perform rustfmt on save.
