/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    ---------------------------------------------------------------------------

    run_headless.rs - Implement the main procedure for headless mode.

*/

use marty_core::{
    floppy_manager::FloppyManager,
    machine::{ExecutionControl, ExecutionState, Machine},
    machine_manager::MACHINE_DESCS,
    rom_manager::RomManager,
    sound::SoundPlayer,
};

use config_toml_bpaf::ConfigFileParams;
use marty_core::{
    coreconfig::CoreConfig,
    videocard::{ClockingMode, VideoType},
};

pub fn run_headless(config: &ConfigFileParams, rom_manager: RomManager, _floppy_manager: FloppyManager) {
    // Init sound
    // The cpal sound library uses generics to initialize depending on the SampleFormat type.
    // On Windows at least a sample type of f32 is typical, but just in case...
    let sample_fmt = SoundPlayer::get_sample_format();
    let sp = match sample_fmt {
        cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(),
        cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(),
        cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(),
    };

    // Look up the machine description given the machine type in the configuration file
    let machine_desc_opt = MACHINE_DESCS.get(&config.machine.model);
    if let Some(machine_desc) = machine_desc_opt {
        log::debug!(
            "Given machine type {:?} got machine description: {:?}",
            config.machine.model,
            machine_desc
        );
    }
    else {
        log::error!("Couldn't get machine description for {:?}", config.machine.model);

        eprintln!(
            "Couldn't get machine description for machine type {:?}. \
             Check that you have a valid machine type specified in configuration file.",
            config.machine.model
        );
        std::process::exit(1);
    }

    let (video_type, _clock_mode, _video_debug) = {
        let mut video_type: Option<VideoType> = None;
        let mut clock_mode: Option<ClockingMode> = None;
        let video_cards = config.get_video_cards();
        if video_cards.len() > 0 {
            clock_mode = video_cards[0].clocking_mode;
            video_type = Some(video_cards[0].video_type); // Videotype is not optional
        }
        (
            video_type.unwrap_or(VideoType::CGA),
            clock_mode.unwrap_or_default(),
            video_cards[0].debug.unwrap_or(false),
        )
    };

    // Instantiate the main Machine data struct
    // Machine coordinates all the parts of the emulated computer
    let mut machine = Machine::new(
        config,
        config.machine.model,
        *machine_desc_opt.unwrap(),
        config.emulator.trace_mode.unwrap_or_default(),
        video_type,
        sp,
        rom_manager,
    );

    // Load program binary if one was specified in config options
    if let Some(prog_bin) = &config.emulator.run_bin {
        if let Some(prog_seg) = config.emulator.run_bin_seg {
            if let Some(prog_ofs) = config.emulator.run_bin_ofs {
                let prog_vec = match std::fs::read(prog_bin.clone()) {
                    Ok(vec) => vec,
                    Err(e) => {
                        eprintln!("Error opening filename {:?}: {}", prog_bin, e);
                        std::process::exit(1);
                    }
                };

                if let Err(_) = machine.load_program(&prog_vec, prog_seg, prog_ofs) {
                    eprintln!(
                        "Error loading program into memory at {:04X}:{:04X}.",
                        prog_seg, prog_ofs
                    );
                    std::process::exit(1);
                };
            }
            else {
                eprintln!("Must specifiy program load offset.");
                std::process::exit(1);
            }
        }
        else {
            eprintln!("Must specifiy program load segment.");
            std::process::exit(1);
        }
    }

    let mut exec_control = ExecutionControl::new();
    exec_control.set_state(ExecutionState::Running);

    loop {
        // This should really return a Result
        machine.run(1000, &mut exec_control);
    }

    //std::process::exit(0);
}
