#![no_std]
#![allow(static_mut_refs)]
#![feature(naked_functions)]

extern crate alloc;

#[cfg(target_os = "horizon")]
mod allocator;

mod crystal;
mod draw;
mod gen6;
mod gen7;
mod pnp;
mod rng;
mod time_lock;
mod title;
mod transporter;
mod utils;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HASH: &str = env!("GIT_HASH");

use title::{loaded_title, LoadedTitle, TitleError};

#[cfg(target_os = "horizon")]
#[panic_handler]
fn my_panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        let file = location.file();
        let slice = &file[file.len() - 7..];

        // Since we're about to break, storing a few u32s in these registers won't break us further.
        // In the future it might be helpful to disable this for release builds.
        unsafe {
            // r9 and r10 aren't used as frequently as the lower registers, so in most situations
            // we'll get more useful information by storing the last 4 characters of the file name
            // and the line number where we broke.
            let partial_file_name = *(slice.as_ptr() as *const u32);
            core::arch::asm!("mov r9, {}", in(reg) partial_file_name);
            core::arch::asm!("mov r10, {}", in(reg) location.line());
        }
    }

    // svcBreak(USERBREAK_PANIC)
    unsafe { core::arch::asm!("svc 0x3C", in("r0") 0u32) };
    loop {}
}

fn initialize_loaded_title(title: &LoadedTitle) {
    match title {
        LoadedTitle::S | LoadedTitle::M => gen7::init_sm(),
        LoadedTitle::Us => gen7::init_us(),
        LoadedTitle::Um => gen7::init_um(),
        LoadedTitle::Or | LoadedTitle::As => gen6::init_oras(),
        LoadedTitle::X | LoadedTitle::Y => gen6::init_xy(),
        LoadedTitle::Transporter => transporter::init_transporter(),
        LoadedTitle::CrystalEn
        | LoadedTitle::CrystalDe
        | LoadedTitle::CrystalFr
        | LoadedTitle::CrystalEs
        | LoadedTitle::CrystalIt => crystal::init_crystal(),
    }
}

#[cfg(target_os = "horizon")]
#[no_mangle]
pub extern "C" fn initialize() {
    if let Ok(title) = loaded_title() {
        if time_lock_core::preset::TITLES
            .iter()
            .any(|(id, _, _)| *id == *title as u64)
        {
            time_lock::initialize();
        }
        initialize_loaded_title(title);
    }
}

/// 0=unrelated title, 1=allowed, 2=version/fingerprint mismatch, 3=hook mismatch.
/// Called before C installs any overlay or game-specific hooks.
#[no_mangle]
pub extern "C" fn reader_startup_status() -> u32 {
    let title = match loaded_title() {
        Ok(title) => title,
        Err(TitleError::InvalidTitle) => return 0,
        Err(_) => return 2,
    };
    let (branches, nop): (&[u32], Option<u32>) = match title {
        LoadedTitle::X | LoadedTitle::Y => (&[0x11e880], Some(0x1254f8)),
        LoadedTitle::Or | LoadedTitle::As => (&[0x1122fc], Some(0x125ec8)),
        LoadedTitle::S | LoadedTitle::M => (&[0x359784, 0x3eab60], None),
        LoadedTitle::Us => (&[0x361e60, 0x3fcbbc], None),
        LoadedTitle::Um => (&[0x361e60, 0x3fcbc0], None),
        _ => return 1,
    };
    for &address in branches {
        if !pnp::is_memory_mapped(address) || pnp::read::<u32>(address) >> 24 != 0xeb {
            return 3;
        }
    }
    if let Some(address) = nop {
        if !pnp::is_memory_mapped(address) {
            return 3;
        }
        let word: u32 = pnp::read(address);
        if word != 0xe1a00000 && word != 0xe320f000 {
            return 3;
        }
    }
    1
}

fn run_loaded_title_frame(title: &LoadedTitle) {
    match title {
        LoadedTitle::S | LoadedTitle::M => gen7::run_sm_frame(),
        LoadedTitle::Us | LoadedTitle::Um => gen7::run_usum_frame(),
        LoadedTitle::Or | LoadedTitle::As => gen6::run_oras_frame(),
        LoadedTitle::X | LoadedTitle::Y => gen6::run_xy_frame(),
        LoadedTitle::Transporter => transporter::run_frame(),
        LoadedTitle::CrystalEn
        | LoadedTitle::CrystalDe
        | LoadedTitle::CrystalFr
        | LoadedTitle::CrystalEs
        | LoadedTitle::CrystalIt => crystal::run_frame(),
    }
}

#[no_mangle]
pub extern "C" fn run_frame() {
    match loaded_title() {
        Ok(title) => run_loaded_title_frame(title),
        Err(TitleError::InvalidUpdate {
            remaster_version,
            debug_info,
            is_citra,
        }) => {
            pnp::println!("Unsupported game update!");
            pnp::println!("");
            pnp::println!("Please update your game");
            pnp::println!("for PokeReader to run");
            pnp::println!("");
            pnp::println!("Detected info:");
            pnp::println!(
                "Playing on {}",
                match is_citra {
                    true => "Citra",
                    false => "Real hardware",
                }
            );
            pnp::println!("Update ver {}", remaster_version);
            pnp::println!("Debug: {}", debug_info.as_deref().unwrap_or_default());
            pnp::println!("");
            pnp::println!("PokeReader version:");
            pnp::println!("{} {}", VERSION, GIT_HASH);
        }
        Err(TitleError::InvalidTitle) => {}
    }
}
