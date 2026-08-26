use web_time::{Duration, Instant};

// Rendering Stats
pub struct Counter {
    pub frame_count: u64,
    pub cycle_count: u64,
    pub instr_count: u64,

    pub current_ups: u32,
    pub current_cps: u64,
    pub current_fps: u32,
    pub current_ips: u64,
    pub emulated_fps: u32,
    pub current_emulated_frames: u64,
    pub emulated_frames: u64,

    pub ups: u32,
    pub fps: u32,
    pub last_frame: Instant,
    #[allow(dead_code)]
    pub last_sndbuf: Instant,
    pub last_second: Instant,
    pub last_cpu_cycles: u64,
    pub current_cpu_cps: u64,
    pub last_system_ticks: u64,
    pub last_pit_ticks: u64,
    pub current_sys_tps: u64,
    pub current_pit_tps: u64,
    pub emulation_time: Duration,
    pub render_time: Duration,
    pub accumulated_us: u128,
    pub cpu_mhz: f64,
    pub cycles_per_frame: u32,
    pub cycle_target: u32,
}

impl Counter {
    fn new() -> Self {
        Self {
            frame_count: 0,
            cycle_count: 0,
            instr_count: 0,

            current_ups: 0,
            current_cps: 0,
            current_fps: 0,
            current_ips: 0,

            emulated_fps: 0,
            current_emulated_frames: 0,
            emulated_frames: 0,

            ups: 0,
            fps: 0,
            last_second: Instant::now(),
            last_sndbuf: Instant::now(),
            last_frame: Instant::now(),
            last_cpu_cycles: 0,
            current_cpu_cps: 0,
            last_system_ticks: 0,
            last_pit_ticks: 0,
            current_sys_tps: 0,
            current_pit_tps: 0,
            emulation_time: Duration::ZERO,
            render_time: Duration::ZERO,
            accumulated_us: 0,
            cpu_mhz: 0.0,
            cycles_per_frame: 0,
            cycle_target: 0,
        }
    }
}
