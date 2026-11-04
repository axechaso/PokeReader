use crate::utils;

static mut TINYMT_SEED: u32 = 0;

pub fn tinymt_seed() -> u32 {
    unsafe { TINYMT_SEED }
}

fn init_tinymt_hook(regs: &[u32], _stack_pointer: *mut u32) {
    if regs[0] == 0x8c52808 || regs[0] == 0x8c59e04 {
        unsafe { TINYMT_SEED = regs[1] };
    }
}

fn init_xy_hooks() {
    utils::hook_game_branch! {
        game_name = xy,
        init_tinymt_hook = 0x11e880,
    }
}

fn init_y_hooks() {
    if crate::pnp::is_memory_mapped(0x10da38) && crate::pnp::read::<u32>(0x10da38) == 0xeb00e54e {
        crate::time_lock::activate_saved_y_game_time_lock();
        utils::hook_game_branch_skip! {
            game_name = xy,
            normal = init_tinymt_hook = 0x11e880,
            skip = seed_time_hook = 0x10da38,
        }
    } else {
        init_xy_hooks();
    }
}

pub fn init_xy() {
    utils::hook_gen6_mt_seed(0x1254f8);
    if crate::pnp::title_id() == 0x0004000000055e00 {
        init_y_hooks();
    } else {
        init_xy_hooks();
    }
}

fn seed_time_hook(regs: &mut [u32], _stack_pointer: *mut u32) -> bool {
    crate::time_lock::override_y_seed_time_call(regs)
}

pub fn init_oras() {
    utils::hook_gen6_mt_seed(0x125ec8);
    utils::hook_game_branch! {
        game_name = oras,
        init_tinymt_hook = 0x1122fc,
    }
}
