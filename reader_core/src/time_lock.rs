//! Development UI and read-only seed diagnostics. No retail clock adapter is enabled.
use crate::pnp::{self, Button};
use alloc::{format, string::String};
use core::fmt::Write;
use time_lock_core::{
    clock::{Clock, Mode},
    date::{from_1900_ms, DateTime},
    preset::{Preset, MAX_PRESET_BYTES, TITLES},
    trace::{Event, Observation, Trace},
};

extern "C" {
    fn host_tl_tick() -> u64;
    fn host_tl_real_ms() -> i64;
    fn host_tl_lock();
    fn host_tl_unlock();
    fn host_tl_load(title: u64, backup: u32, out: *mut u8, capacity: u32) -> i32;
    fn host_tl_save(title: u64, bytes: *const u8, length: u32) -> i32;
    fn host_tl_export(title: u64, session: u64, bytes: *const u8, length: u32) -> i32;
}

const TICK_HZ: u32 = 268_123_480;
const TRACE_CAPACITY: usize = 32;

struct Guard;
impl Guard {
    fn lock() -> Self {
        unsafe { host_tl_lock() };
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe { host_tl_unlock() };
    }
}

static mut TRACE: Trace<TRACE_CAPACITY> = Trace::new();
static mut SESSION: u64 = 0;

fn tick() -> u64 {
    unsafe { host_tl_tick() }
}
fn real_ms() -> i64 {
    unsafe { host_tl_real_ms() }
}

fn observe(event: Event, game_ms: i64, value: u64) {
    // Read OS time outside the short state lock. Never allocate or write SD here.
    let tick = tick();
    let real_ms = real_ms();
    let _guard = Guard::lock();
    unsafe {
        TRACE.push(Observation {
            event,
            session: 1,
            tick,
            real_ms,
            game_ms,
            value,
        });
    }
}

pub fn initialize() {
    unsafe {
        SESSION = tick();
    }
    observe(Event::Startup, -1, pnp::title_id());
}

/// This is the existing Gen 7 seeding context, NOT a proven calendar-read hook.
pub fn observe_seed_input(game_ms: u64, seed_ticks: u32) {
    observe(
        Event::SeedInput,
        i64::try_from(game_ms).unwrap_or(-1),
        u64::from(seed_ticks),
    );
}

pub fn observe_seed(seed: u32) {
    observe(Event::SeedObserved, -1, u64::from(seed));
}

fn load_preset(title: u64) -> (Preset, &'static str) {
    let mut buffer = [0u8; MAX_PRESET_BYTES];
    let mut had_error = false;
    for backup in 0..=1 {
        let count = unsafe { host_tl_load(title, backup, buffer.as_mut_ptr(), buffer.len() as u32) };
        if count > 0 && count as usize <= buffer.len() {
            if let Ok(preset) = Preset::parse(&buffer[..count as usize], title) {
                return (
                    preset,
                    if backup == 0 {
                        "Preset loaded"
                    } else {
                        "Recovered backup; save to repair"
                    },
                );
            }
            had_error = true;
        } else if count < 0 {
            had_error = true;
        }
    }
    let mut preset = Preset::new(title);
    if let Ok(now) = from_1900_ms(real_ms()).and_then(DateTime::from_epoch_ms) {
        preset.target = now;
    }
    (
        preset,
        if had_error {
            "Preset invalid/read failed; disabled"
        } else {
            "New preset (not saved)"
        },
    )
}

fn save_preset(preset: Preset) -> bool {
    if preset.validate().is_err() {
        return false;
    }
    let bytes = format!("{}", preset);
    unsafe { host_tl_save(preset.title_id, bytes.as_ptr(), bytes.len() as u32) == 0 }
}

fn export_trace() -> bool {
    let (trace, session) = {
        let _guard = Guard::lock();
        unsafe { (TRACE, SESSION) }
    };
    let mut text = format!("# PokeReader {} {}\n# title={:016X},remaster={},session={}\n# game_clock=original,adapter=unverified,dropped={}\n# real_ms_epoch=1900,game_ms_epoch=2000,unknown_game_ms=-1\nevent,session,tick,real_ms,game_ms,value\n",
        crate::VERSION, crate::GIT_HASH, pnp::title_id(), pnp::update_version(), session, trace.dropped());
    for entry in trace.entries() {
        let _ = writeln!(
            text,
            "{:?},{},{},{},{},{}",
            entry.event, entry.session, entry.tick, entry.real_ms, entry.game_ms, entry.value
        );
    }
    unsafe { host_tl_export(pnp::title_id(), session, text.as_ptr(), text.len() as u32) == 0 }
}

const FIELDS: [&str; 11] = [
    "Year",
    "Month",
    "Day",
    "Hour",
    "Minute",
    "Second",
    "Millisecond",
    "Request next boot",
    "Initial mode",
    "Reuse on full boot",
    "Preview step (ms)",
];
const STEPS: [i64; 6] = [-60_000, -1000, -1, 1, 1000, 60_000];

struct Editor {
    preset: Preset,
    preview: Clock,
    field: usize,
    step: usize,
    message: &'static str,
    dirty: bool,
    fault: bool,
}

impl Editor {
    fn new() -> Self {
        let (preset, message) = load_preset(pnp::title_id());
        let mut preview = Clock::new(TICK_HZ).unwrap();
        let _ = preview.start_session(
            preset.target.to_epoch_ms().unwrap_or(0),
            preset.initial_mode,
            tick(),
        );
        Self {
            preset,
            preview,
            field: 0,
            step: 3,
            message,
            dirty: false,
            fault: false,
        }
    }

    fn edit(&mut self, delta: i16) {
        if self.field < 7 {
            match self.preset.target.edit(self.field, delta) {
                Ok(target) => self.preset.target = target,
                Err(_) => {
                    self.message = "Invalid date/range; unchanged";
                    return;
                }
            }
        } else {
            match self.field {
                7 => self.preset.enabled = !self.preset.enabled,
                8 => {
                    self.preset.initial_mode = if self.preset.initial_mode == Mode::Frozen {
                        Mode::Running
                    } else {
                        Mode::Frozen
                    }
                }
                9 => self.preset.reuse = !self.preset.reuse,
                _ => {
                    self.step = (self.step as i16 + delta).rem_euclid(STEPS.len() as i16) as usize;
                    return;
                }
            }
        }
        self.dirty = true;
        self.message = "Edited; A saves next-boot preset";
        // Editing a next-boot preset never changes the current preview or game.
    }

    fn update(&mut self) {
        if pnp::is_just_pressed(Button::L) {
            self.field = (self.field + FIELDS.len() - 1) % FIELDS.len();
        } else if pnp::is_just_pressed(Button::R) {
            self.field = (self.field + 1) % FIELDS.len();
        } else if pnp::is_just_pressed(Button::Dup) {
            self.edit(1);
        } else if pnp::is_just_pressed(Button::Ddown) {
            self.edit(-1);
        } else if pnp::is_just_pressed(Button::A) {
            if save_preset(self.preset) {
                self.message = "Saved; adapter still unavailable";
                self.dirty = false;
            } else {
                self.message = "Save failed; previous preset kept";
            }
        } else if pnp::is_just_pressed(Button::X) {
            let result = if self.fault {
                Err(time_lock_core::clock::ClockError::TickRegression)
            } else if self.preview.mode() == Mode::Frozen {
                self.preview.release(tick())
            } else {
                self.preview.freeze(tick())
            };
            self.message = if result.is_ok() {
                "Preview only; game unchanged"
            } else {
                "Clock fault; SELECT resets preview"
            };
            observe(Event::PreviewCommand, -1, 0);
        } else if pnp::is_just_pressed(Button::B) {
            self.message = if self.preview.step(STEPS[self.step]).is_ok() {
                "Preview step applied"
            } else {
                "Step rejected; freeze preview first"
            };
        } else if pnp::is_just_pressed(Button::Select) {
            let result = self.preview.start_session(
                self.preset.target.to_epoch_ms().unwrap_or(0),
                self.preset.initial_mode,
                tick(),
            );
            self.fault = result.is_err();
            self.message = "Preview reset; game unchanged";
        } else if pnp::is_just_pressed(Button::Y) {
            self.message = if export_trace() {
                "Log exported to time-lock/logs"
            } else {
                "Log export failed"
            };
        }
    }

    fn draw(&mut self, is_locked: bool) {
        pnp::set_print_max_len(40);
        pnp::println!("Time control - DEVELOPMENT");
        pnp::println!("Game clock: ORIGINAL (no adapter)");
        pnp::println!(
            "Next boot{}: {}",
            if self.dirty { "*" } else { "" },
            self.preset.target
        );
        pnp::println!(
            "Request:{}  {:?}  Reuse:{}",
            self.preset.enabled,
            self.preset.initial_mode,
            self.preset.reuse
        );
        let now = self.preview.read(tick());
        match now.and_then(|ms| DateTime::from_epoch_ms(ms).map_err(Into::into)) {
            Ok(date) if !self.fault => pnp::println!("Preview {:?}: {}", self.preview.mode(), date),
            _ => {
                self.fault = true;
                pnp::println!("Preview clock fault: SELECT to reset");
            }
        }
        pnp::println!("Field: {}", FIELDS[self.field]);
        pnp::println!("Step: {:+} ms (preview)", STEPS[self.step]);
        pnp::println!("L/R field; Up/Down edit");
        pnp::println!("A save; X preview freeze/run");
        pnp::println!("B step; SELECT reset; Y export log");
        pnp::println!("Penalty: observe only; soft reset: off");
        pnp::println!("{}", self.message);
        if is_locked {
            pnp::println!("Inputs locked (X+Y to unlock)");
        }
    }
}

pub fn update_and_draw(is_locked: bool) {
    // Same frame-thread ownership as the existing PokeReader menus.
    static mut EDITOR: Option<Editor> = None;
    let editor = unsafe { EDITOR.get_or_insert_with(Editor::new) };
    if !is_locked {
        editor.update();
    }
    editor.draw(is_locked);
}

/// Shared preset editor ABI for the native 3DS configurator. Returns byte count or -1.
/// field=-1 validates/reformats, 0..6 edits date fields, 7..9 toggles settings.
#[no_mangle]
pub unsafe extern "C" fn tl_preset_edit(
    title: u64,
    input: *const u8,
    length: u32,
    field: i32,
    delta: i32,
    output: *mut u8,
    capacity: u32,
) -> i32 {
    if input.is_null() || output.is_null() || length as usize > MAX_PRESET_BYTES {
        return -1;
    }
    let bytes = core::slice::from_raw_parts(input, length as usize);
    let mut preset = match Preset::parse(bytes, title) {
        Ok(p) => p,
        Err(_) => return -1,
    };
    if (0..=6).contains(&field) {
        let delta = match i16::try_from(delta) {
            Ok(n) => n,
            Err(_) => return -1,
        };
        preset.target = match preset.target.edit(field as usize, delta) {
            Ok(d) => d,
            Err(_) => return -1,
        };
    } else {
        match field {
            -1 => {}
            7 => preset.enabled = !preset.enabled,
            8 => {
                preset.initial_mode = if preset.initial_mode == Mode::Frozen {
                    Mode::Running
                } else {
                    Mode::Frozen
                }
            }
            9 => preset.reuse = !preset.reuse,
            _ => return -1,
        }
    }
    copy_preset(preset, output, capacity)
}

unsafe fn copy_preset(preset: Preset, output: *mut u8, capacity: u32) -> i32 {
    if preset.validate().is_err() || output.is_null() {
        return -1;
    }
    let text: String = format!("{}", preset);
    if text.len() > capacity as usize {
        return -1;
    }
    core::ptr::copy_nonoverlapping(text.as_ptr(), output, text.len());
    text.len() as i32
}

#[no_mangle]
pub unsafe extern "C" fn tl_preset_default(
    title: u64,
    now_1900_ms: i64,
    output: *mut u8,
    capacity: u32,
) -> i32 {
    if !TITLES.iter().any(|(id, _, _)| *id == title) {
        return -1;
    }
    let mut preset = Preset::new(title);
    if let Ok(date) = from_1900_ms(now_1900_ms).and_then(DateTime::from_epoch_ms) {
        preset.target = date;
    }
    copy_preset(preset, output, capacity)
}

#[cfg(feature = "test_stubs")]
mod stubs {
    #[no_mangle]
    extern "C" fn host_tl_tick() -> u64 {
        0
    }
    #[no_mangle]
    extern "C" fn host_tl_real_ms() -> i64 {
        0
    }
    #[no_mangle]
    extern "C" fn host_tl_lock() {}
    #[no_mangle]
    extern "C" fn host_tl_unlock() {}
    #[no_mangle]
    extern "C" fn host_tl_load(_: u64, _: u32, _: *mut u8, _: u32) -> i32 {
        0
    }
    #[no_mangle]
    extern "C" fn host_tl_save(_: u64, _: *const u8, _: u32) -> i32 {
        -1
    }
    #[no_mangle]
    extern "C" fn host_tl_export(_: u64, _: u64, _: *const u8, _: u32) -> i32 {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurator_uses_the_same_strict_parser() {
        for (title, _, _) in TITLES {
            let mut input = [0u8; MAX_PRESET_BYTES];
            let mut output = [0u8; MAX_PRESET_BYTES];
            let count = unsafe { tl_preset_default(title, 3_155_673_600_000, input.as_mut_ptr(), 512) };
            assert!(count > 0);
            let old = Preset::parse(&input[..count as usize], title).unwrap();
            assert!(!old.enabled);
            let edited = unsafe {
                tl_preset_edit(
                    title,
                    input.as_ptr(),
                    count as u32,
                    0,
                    1,
                    output.as_mut_ptr(),
                    512,
                )
            };
            assert!(edited > 0);
            assert_eq!(
                Preset::parse(&output[..edited as usize], title)
                    .unwrap()
                    .target
                    .year,
                2001
            );
            assert_eq!(Preset::parse(&input[..count as usize], title), Ok(old));
            assert_eq!(
                unsafe {
                    tl_preset_edit(
                        title,
                        input.as_ptr(),
                        count as u32,
                        0,
                        -1,
                        output.as_mut_ptr(),
                        512,
                    )
                },
                -1
            );
        }
    }

    #[test]
    fn ffi_rejects_invalid_buffers_and_presets_without_writing_output() {
        let mut output = [0xabu8; MAX_PRESET_BYTES];
        let input = b"schema_version=1\nenabled=true\n";
        assert_eq!(
            unsafe {
                tl_preset_edit(
                    TITLES[0].0,
                    input.as_ptr(),
                    input.len() as u32,
                    -1,
                    0,
                    output.as_mut_ptr(),
                    512,
                )
            },
            -1
        );
        assert_eq!(
            unsafe { tl_preset_edit(TITLES[0].0, core::ptr::null(), 0, -1, 0, output.as_mut_ptr(), 512) },
            -1
        );
        assert_eq!(
            unsafe { tl_preset_default(TITLES[0].0, 0, output.as_mut_ptr(), 1) },
            -1
        );
        assert_eq!(unsafe { tl_preset_default(0, 0, output.as_mut_ptr(), 512) }, -1);
        assert_eq!(output, [0xab; MAX_PRESET_BYTES]);
    }
}
