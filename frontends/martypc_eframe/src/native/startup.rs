use std::{cell::RefCell, path::PathBuf, rc::Rc};

use crate::{
    counter::Counter,
    emulator::{
        joystick_state::JoystickData,
        keyboard_state::KeyboardData,
        mouse_state::MouseData,
        EmuFlags,
        Emulator,
    },
    input::HotkeyManager,
    sound::sound_player::SoundInterface,
    MARTY_ICON,
};
use config_toml_bpaf::TestMode;
use display_manager_eframe::EFrameDisplayManagerBuilder;
use frontend_common::{
    cartridge_manager::CartridgeManager,
    display_manager::DmGuiOptions,
    floppy_manager::FloppyManager,
    resource_manager::ResourceManager,
    timestep_manager::TimestepManager,
    vhd_manager::VhdManager,
};
use marty_core::{
    cpu_validator::ValidatorType,
    machine::{ExecutionControl, ExecutionState, MachineBuilder},
    supported_floppy_extensions,
};
use marty_egui::state::GuiState;

pub fn startup(ctx: egui::Context) -> Emulator {
    // TODO: Move most of everything from here into an EmulatorBuilder

    // First we resolve the emulator configuration by parsing the configuration toml and merging it with
    // command line arguments. For the desktop frontend, this is handled by the config_toml_bpaf front end
    // library.
    let config = match config_toml_bpaf::get_config("./martypc.toml") {
        Ok(config) => config,
        Err(e) => match e.downcast_ref::<std::io::Error>() {
            Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "Configuration file not found! Please create martypc.toml in the emulator directory \
                               or provide the path to configuration file with --configfile."
                );

                std::process::exit(1);
            }
            Some(e) => {
                eprintln!("Unknown IO error reading configuration file:\n{}", e);
                std::process::exit(1);
            }
            None => {
                eprintln!(
                    "Failed to parse configuration file. There may be a typo or otherwise invalid toml:\n{}",
                    e
                );
                std::process::exit(1);
            }
        },
    };

    // Now that we have our configuration, we can instantiate a ResourceManager.
    let mut resource_manager = ResourceManager::from_config(config.emulator.basedir.clone(), &config.emulator.paths)
        .unwrap_or_else(|e| {
            log::error!("Failed to create resource manager: {:?}", e);
            std::process::exit(1);
        });

    let resolved_paths = resource_manager.pm.dump_paths();
    for path in &resolved_paths {
        println!("Resolved resource path: {:?}", path);
    }

    // Tell the resource manager to ignore specified dirs
    if let Some(ignore_dirs) = &config.emulator.ignore_dirs {
        resource_manager.set_ignore_dirs(ignore_dirs.clone());
    }

    #[cfg(feature = "cpu_validator")]
    match config.validator.vtype {
        Some(ValidatorType::None) | None => {
            eprintln!("Compiled with validator but no validator specified");
            std::process::exit(1);
        }
        _ => {}
    }

    // Instantiate the new machine manager to load Machine configurations.
    let mut machine_manager = frontend_common::machine_manager::MachineManager::new();
    if let Err(err) = machine_manager.load_configs(&resource_manager) {
        eprintln!("Error loading Machine configuration files: {}", err);
        std::process::exit(1);
    }

    // Initialize machine configuration name, options and prefer_oem flag.
    // If benchmark_mode is true, we use the values from the benchmark configuration section. This
    // gives us the ability to run benchmarks with a consistent, static configuration.
    let mut init_config_name = config.machine.config_name.clone();
    let mut init_prefer_oem = config.machine.prefer_oem;
    let mut init_config_overlays = config.machine.config_overlays.clone().unwrap_or_default();

    if config.emulator.benchmark_mode {
        init_config_name = config.emulator.benchmark.config_name.clone();
        init_prefer_oem = config.emulator.benchmark.prefer_oem;
        init_config_overlays = config.emulator.benchmark.config_overlays.clone().unwrap_or_default();

        println!(
            "Benchmark mode enabled. Using machine config: {} config overlays: [{}] prefer_oem: {}",
            init_config_name,
            init_config_overlays.join(", "),
            init_prefer_oem
        );
    }

    // Get a list of machine configuration names
    let machine_names = machine_manager.get_config_names();
    let have_machine_config = machine_names.contains(&init_config_name);

    // Do --machinescan commandline argument. We print machine info (and rom info if --romscan
    // was also specified) and then quit.
    if config.emulator.machinescan {
        // Print the list of machine configurations and their rom requirements
        for machine in machine_names {
            println!("Machine: {}", machine);
            if let Some(reqs) = machine_manager
                .get_config(&machine)
                .and_then(|config| Some(config.get_rom_requirements()))
            {
                println!("  Requires: {:?}", reqs);
            }
        }

        if !have_machine_config {
            println!("Warning! No matching configuration found for: {}", init_config_name);
            std::process::exit(1);
        }

        // Exit unless we will also run romscan
        if !config.emulator.romscan {
            std::process::exit(0);
        }
    }

    if !have_machine_config {
        eprintln!(
            "No machine configuration for specified config name: {}",
            init_config_name
        );
        std::process::exit(1);
    }

    // Instantiate the new rom manager to load roms
    let mut rom_manager = frontend_common::rom_manager::RomManager::new(init_prefer_oem);
    if let Err(err) = rom_manager.load_defs(&resource_manager) {
        eprintln!("Error loading ROM definition files: {}", err);
        std::process::exit(1);
    }

    // Get the ROM requirements for the requested machine type
    let machine_config_file = {
        for overlay in init_config_overlays.iter() {
            log::debug!("Have machine config overlay from global config: {}", overlay);
        }
        let overlay_vec = init_config_overlays.clone();

        match machine_manager.get_config_with_overlays(&init_config_name, &overlay_vec) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error getting machine config: {}", err);
                std::process::exit(1);
            }
        }
    };
    let (required_features, optional_features) = machine_config_file.get_rom_requirements().unwrap_or_else(|e| {
        eprintln!("Error getting ROM requirements for machine: {}", e);
        std::process::exit(1);
    });

    // Scan the rom resource director(ies)
    if let Err(err) = rom_manager.scan(&resource_manager) {
        eprintln!("Error scanning ROM resource directories: {}", err);
        std::process::exit(1);
    }

    // Determine what complete ROM sets we have
    if let Err(err) = rom_manager.resolve_rom_sets() {
        eprintln!("Error resolving ROM sets: {}", err);
        std::process::exit(1);
    }

    // Do --romscan option.  We print rom and machine info and quit.
    if config.emulator.romscan {
        rom_manager.print_rom_stats();
        rom_manager.print_romset_stats();
        std::process::exit(0);
    }

    println!(
        "Selected machine config {} requires the following ROM features:",
        init_config_name
    );
    for rom_feature in &required_features {
        println!("  {}", rom_feature);
    }

    println!(
        "Selected machine config {} optionally requests the following ROM features:",
        init_config_name
    );
    for rom_feature in &optional_features {
        println!("  {}", rom_feature);
    }

    // Determine if the machine configuration specifies a particular ROM set
    let specified_rom_set = machine_config_file.get_specified_rom_set();

    // Resolve the ROM requirements for the requested ROM features
    let rom_sets_resolved = rom_manager
        .resolve_requirements(required_features, optional_features, specified_rom_set)
        .unwrap_or_else(|err| {
            eprintln!("Error resolving ROM sets for machine: {}", err);
            std::process::exit(1);
        });

    println!(
        "Selected machine config {} has resolved the following ROM sets:",
        init_config_name
    );
    for rom_set in &rom_sets_resolved {
        println!("  {}", rom_set);
    }

    // Create the ROM manifest
    let rom_manifest = rom_manager
        .create_manifest(rom_sets_resolved.clone(), &resource_manager)
        .unwrap_or_else(|err| {
            eprintln!("Error loading ROM set: {}", err);
            std::process::exit(1);
        });

    log::debug!("Created manifest!");
    for (i, rom) in rom_manifest.roms.iter().enumerate() {
        log::debug!("  rom {}: md5: {} length: {}", i, rom.md5, rom.data.len());
    }

    // Instantiate the floppy manager
    let mut floppy_manager = FloppyManager::new();

    let mut floppy_extensions = config
        .emulator
        .media
        .raw_sector_image_extensions
        .clone()
        .unwrap_or_default();
    let managed_extensions = supported_floppy_extensions();
    log::debug!(
        "marty_core reports native support for the following extensions: {:?}",
        managed_extensions
    );
    managed_extensions.iter().for_each(|ext| {
        if !floppy_extensions.contains(&ext.to_string()) {
            floppy_extensions.push(ext.to_string());
        }
    });
    floppy_manager.set_extensions(Some(floppy_extensions));

    // Scan the "floppy" resource
    if let Err(e) = floppy_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read floppy path: {:?}", e);
        std::process::exit(1);
    }

    // Scan the "autofloppy" resource
    if let Err(e) = floppy_manager.scan_autofloppy(&resource_manager) {
        eprintln!("Failed to read autofloppy path: {:?}", e);
        std::process::exit(1);
    }

    // Instantiate the VHD manager
    let mut vhd_manager = VhdManager::new();

    // Scan the "hdd" resource
    if let Err(e) = vhd_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read hdd path: {:?}", e);
        std::process::exit(1);
    }

    // Instantiate the cartridge manager
    let mut cart_manager = CartridgeManager::new();

    // Scan the "cartridge" resource
    if let Err(e) = cart_manager.scan_resource(&resource_manager) {
        eprintln!("Failed to read cartridge path: {:?}", e);
        std::process::exit(1);
    }

    // Enumerate host serial ports
    #[cfg(feature = "serial")]
    let serial_ports = {
        let ports = serialport::available_ports().unwrap_or_else(|e| {
            log::warn!("Didn't find any serial ports: {:?}", e);
            Vec::new()
        });

        for port in &serial_ports {
            log::debug!("Found serial port: {:?}", port);
        }
        ports
    };

    log::debug!("Test mode: {:?}", config.tests.test_mode);

    // // If fuzzer mode was specified, run the emulator in fuzzer mode now
    // #[cfg(feature = "arduino_validator")]
    // if config.emulator.fuzzer {
    //     return run_fuzzer(&config);
    // }
    //
    // // If test generate mode was specified, run the emulator in test generation mode now
    // #[cfg(feature = "cpu_validator")]
    // match config.tests.test_mode {
    //     #[cfg(feature = "arduino_validator")]
    //     Some(TestMode::Generate) => return run_gentests(&config),
    //     #[cfg(not(feature = "arduino_validator"))]
    //     Some(TestMode::Generate) => panic!("Test generation not supported without a validator backend."),
    //     Some(TestMode::Run) | Some(TestMode::Validate) => return run_runtests(config),
    //     #[cfg(feature = "arduino_validator")]
    //     Some(TestMode::Process) => return run_processtests(config),
    //     Some(TestMode::None) | None | _ => {}
    // }
    // #[cfg(not(feature = "cpu_validator"))]
    // {
    //     if !matches!(config.tests.test_mode, None | Some(TestMode::None)) {
    //         eprintln!("Test mode not supported without validator feature.");
    //         std::process::exit(1);
    //     }
    // }
    //
    // if config.emulator.benchmark_mode {
    //     return run_benchmark(
    //         &config,
    //         machine_config_file,
    //         rom_manifest,
    //         resource_manager,
    //         rom_manager,
    //         floppy_manager,
    //     );
    // }

    // If headless mode was specified, run the emulator in headless mode now
    if config.emulator.headless {
        //return run_headless::run_headless(&config, rom_manager, floppy_manager);
    }

    // ----------------------------------------------------------------------------
    // From this point forward, it is assumed we are staring the full GUI frontend!
    // ----------------------------------------------------------------------------

    let mut hotkey_manager = HotkeyManager::new();
    hotkey_manager.add_hotkeys(config.emulator.input.hotkeys.clone());

    // ExecutionControl is shared via RefCell with GUI so that state can be updated by control widget
    let exec_control = Rc::new(RefCell::new(ExecutionControl::new()));

    // Set CPU state to Running if cpu_autostart option was set in config
    if config.emulator.cpu_autostart {
        exec_control.borrow_mut().set_state(ExecutionState::Running);
    }

    let stat_counter = Counter::new();

    // KB modifiers
    let kb_data = KeyboardData::new();

    // Mouse event struct
    let mouse_data = MouseData::new(config.emulator.input.reverse_mouse_buttons);
    log::debug!(
        "Reverse mouse buttons is: {}",
        config.emulator.input.reverse_mouse_buttons
    );

    let mut sound_config = Default::default();
    let mut sound_player = if config.emulator.audio.enabled {
        let mut sound_player = SoundInterface::new(config.emulator.audio.enabled);

        match sound_player.open_device() {
            Ok(_) => {
                log::info!("Opened audio device: {}", sound_player.device_name());
            }
            Err(e) => {
                eprintln!("Failed to open audio device: {:?}", e);
                std::process::exit(1);
            }
        }

        match sound_player.open_stream() {
            Ok(_) => {
                log::info!("Opened audio stream.");
            }
            Err(e) => {
                eprintln!("Failed to open audio stream: {:?}", e);
                std::process::exit(1);
            }
        }

        sound_config = sound_player.config();
        Some(sound_player)
    }
    else {
        None
    };

    // Init sound
    /*
    let sound_player_opt = {
        if config.emulator.audio.enabled {
            // The cpal sound library uses generics to initialize depending on the SampleFormat type.
            // On Windows at least a sample type of f32 is typical, but just in case...
            let (audio_device, sample_fmt) = SoundPlayer::get_device();
            let sp = match sample_fmt {
                cpal::SampleFormat::F32 => SoundPlayer::new::<f32>(audio_device),
                cpal::SampleFormat::I16 => SoundPlayer::new::<i16>(audio_device),
                cpal::SampleFormat::U16 => SoundPlayer::new::<u16>(audio_device),
            };
            Some(sp)
        }
        else {
            None
        }
    };*/

    let machine_config = machine_config_file.to_machine_config();

    let trace_file_base = resource_manager.get_resource_path("trace").unwrap_or_else(|| {
        eprintln!("Failed to retrieve 'trace' resource path.");
        std::process::exit(1);
    });

    let mut trace_file_path = None;
    if let Some(trace_file) = &config.machine.cpu.trace_file {
        log::info!("Using CPU trace log file: {:?}", trace_file);
        trace_file_path = Some(trace_file_base.join(trace_file));
    }

    // Calculate the path to the keyboard layout file
    let mut kb_layout_file_path = None;
    let mut kb_string = "US".to_string();

    if let Some(global_kb_string) = &config.machine.input.keyboard_layout {
        kb_string = global_kb_string.clone()
    }
    else {
        if let Some(keyboard) = machine_config.keyboard.as_ref() {
            kb_string = keyboard.layout.clone();
        }
    }

    if let Some(mut kb_layout_resource_path) = resource_manager.get_resource_path("keyboard_layout") {
        kb_layout_resource_path.push(format!("keyboard_{}.toml", kb_string));
        kb_layout_file_path = Some(kb_layout_resource_path);
    }

    let mut disassembly_file_path = None;
    if let Some(disassembly_file) = config.machine.disassembly_file.as_ref() {
        disassembly_file_path = Some(trace_file_base.join(disassembly_file));
        log::info!(
            "Using disassembly file: {:?}",
            disassembly_file_path.clone().unwrap_or(PathBuf::from("None"))
        );
    }

    let machine_builder = MachineBuilder::new()
        .with_core_config(Box::new(&config))
        .with_machine_config(&machine_config)
        .with_roms(rom_manifest)
        .with_trace_mode(config.machine.cpu.trace_mode.unwrap_or_default())
        .with_trace_log(trace_file_path)
        .with_sound_config(sound_config)
        .with_keyboard_layout(kb_layout_file_path)
        .with_listing_file(disassembly_file_path);

    let machine = machine_builder.build().unwrap_or_else(|e| {
        log::error!("Failed to build machine: {:?}", e);
        std::process::exit(1);
    });

    let sound_sources = machine.get_sound_sources();

    if let Some(si) = sound_player.as_mut() {
        log::debug!("Machine configuration reported {} sound sources", sound_sources.len());
        for source in sound_sources.iter() {
            log::debug!("Adding sound source: {}", source.name);
            if let Err(e) = si.add_source(source) {
                log::error!("Failed to add sound source: {:?}", e);
                std::process::exit(1);
            }
        }

        // PC Speaker is always first sound source. Set its volume to 25%.
        si.set_volume(0, 0.25);
    }

    // Get a list of video devices from machine.
    let cardlist = machine.bus().enumerate_videocards();

    let mut highest_rate = 50;
    for card in cardlist.iter() {
        let rate = machine.bus().video(&card).unwrap().get_refresh_rate();
        if rate > highest_rate {
            highest_rate = rate;
        }
    }

    let gui_options = DmGuiOptions {
        enabled: !config.gui.disabled,
        theme: config.gui.theme,
        menu_theme: config.gui.menu_theme,
        menubar_h: 24, // TODO: Dynamically measure the height of the egui menu bar somehow
        zoom: config.gui.zoom.unwrap_or(1.0),
        debug_drawing: false,
    };

    // Create displays.
    let mut display_manager = EFrameDisplayManagerBuilder::build(
        ctx.clone(),
        &config.emulator.window,
        cardlist,
        &config.emulator.scaler_preset,
        None,
        Some(MARTY_ICON),
        &gui_options,
    )
    .unwrap_or_else(|e| {
        log::error!("Failed to create displays: {:?}", e);
        std::process::exit(1);
    });

    // Create joystick data
    let joy_data = JoystickData::new(
        config.emulator.input.joystick_keys.clone(),
        config.emulator.input.keyboard_joystick,
    );

    // Create GUI state
    let render_egui = true;
    let gui = GuiState::new(exec_control.clone());

    let machine_events = Vec::new();

    let (sender, receiver) = crossbeam_channel::unbounded();

    // Put everything we want to handle in event loop into an Emulator struct
    let mut emu = Emulator {
        rm: resource_manager,
        dm: display_manager,
        romm: rom_manager,
        romsets: rom_sets_resolved.clone(),
        config,
        machine,
        machine_events,
        exec_control,
        mouse_data,
        kb_data,
        joy_data,
        stat_counter,
        gui,
        floppy_manager,
        vhd_manager,
        cart_manager,
        perf: Default::default(),
        flags: EmuFlags {
            render_gui: render_egui,
            debug_keyboard: false,
        },
        hkm: hotkey_manager,
        si: sound_player,
        sender,
        receiver,
    };

    // Resize video cards
    emu.post_dm_build_init();

    // Set list of host serial ports
    #[cfg(feature = "serial")]
    emu.gui.set_host_serial_ports(serial_ports);

    // let adapter_info = emu.dm.get_main_backend().and_then(|backend| backend.get_adapter_info());
    //
    // let (backend_str, adapter_name_str) = {
    //     let backend_str;
    //     let adapter_name_str;
    //
    //     if let Some(adapter_info) = adapter_info {
    //         backend_str = format!("{:?}", adapter_info.backend);
    //         adapter_name_str = format!("{}", adapter_info.name);
    //         (backend_str, adapter_name_str)
    //     }
    //     else {
    //         log::error!("Failed to get adapter info from backend.");
    //         std::process::exit(1);
    //     }
    // };
    //
    // if adapter_name_str.contains("llvmpipe") {
    //     emu.gui.show_warning(
    //         &"MartyPC was unable to initialize a hardware accellerated backend.\n\
    //             MartyPC is running under software rasterization (llvmpipe).\n\
    //             Performance will be poor. (Do you have libx11-dev installed?)"
    //             .to_string(),
    //     );
    // }
    //
    // log::debug!("wgpu using adapter: {}, backend: {}", adapter_name_str, backend_str);

    if let Err(e) = emu.apply_config() {
        log::error!("Failed to apply configuration to Emulator state: {}", e);
        std::process::exit(1);
    }

    if let Err(_e) = emu.mount_vhds() {
        log::error!("Failed to mount VHDs!");
        std::process::exit(1);
    }

    // Return emulator object
    emu

    //
    // let event_loop = emu.dm.take_event_loop();
    //
    // // Run the winit event loop
    // if let Err(_e) = event_loop.run(move |event, elwt| {
    //     handle_thread_event(&mut emu);
    //     handle_event(&mut emu, &mut timestep_manager, event, elwt);
    // }) {
    //     log::error!("Failed to start event loop!");
    //     std::process::exit(1);
    // }
    //
    // std::process::exit(0);
}
