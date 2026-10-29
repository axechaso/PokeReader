#pragma once
#include <3ds.h>

Result host_tl_platform_init(void);
u64 host_tl_tick(void);
s64 host_tl_real_ms(void);
void host_tl_lock(void);
void host_tl_unlock(void);
s32 host_tl_load(u64 title, u32 backup, u8 *out, u32 capacity);
s32 host_tl_save(u64 title, const u8 *bytes, u32 length);
s32 host_tl_export(u64 title, u64 session, const u8 *bytes, u32 length);
void host_tl_message(const char *body);
s32 tl_preset_default(u64 title, s64 now_1900_ms, u8 *out, u32 capacity);
s32 tl_preset_edit(u64 title, const u8 *input, u32 length, s32 field, s32 delta, u8 *out, u32 capacity);
