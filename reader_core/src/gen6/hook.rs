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

pub fn init_xy() {
    utils::hook_gen6_mt_seed(0x1254f8);
    if crate::pnp::title_id() == 0x0004000000055e00
        && crate::pnp::is_memory_mapped(0x1083d4)
        && crate::pnp::read::<u32>(0x1083d4) == 0xeb001594
        && crate::pnp::is_memory_mapped(0x1d4084)
        && crate::pnp::read::<u32>(0x1d4084) == 0xebfce668
        && crate::pnp::is_memory_mapped(0x146f84)
        && crate::pnp::read::<u32>(0x146f84) == 0xebff4a83
    {
        utils::hook_game_branch_seed_regs! {
            game_name = xy,
            normal = init_tinymt_hook = 0x11e880,
            arm_one = arm_initial_seed_hook = 0x1083d4,
            arm_two = arm_save_seed_hook = 0x1d4084,
            skip = os_time_hook = 0x146f84,
        }
    } else {
        utils::hook_game_branch! {
            game_name = xy,
            init_tinymt_hook = 0x11e880,
        }
    }
}

fn arm_initial_seed_hook(_regs: &[u32], _stack_pointer: *mut u32) {
    crate::time_lock::arm_y_seed_time_hook(1);
}

fn arm_save_seed_hook(_regs: &[u32], _stack_pointer: *mut u32) {
    crate::time_lock::arm_y_seed_time_hook(2);
}

fn os_time_hook(regs: &mut [u32], _stack_pointer: *mut u32) -> bool {
    crate::time_lock::override_y_os_time_call(regs)
}

pub fn init_oras() {
    utils::hook_gen6_mt_seed(0x125ec8);
    utils::hook_game_branch! {
        game_name = oras,
        init_tinymt_hook = 0x1122fc,
    }
}
