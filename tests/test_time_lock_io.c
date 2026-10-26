// Exercise the actual SD persistence implementation with a fault-injecting FS.
#include <assert.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include "3ds.h"
#include "time_lock.h"

typedef struct { bool exists; char path[140]; u8 data[16384]; u32 size; } MockFile;
static MockFile files[8];
static jmp_buf power_loss;
static unsigned boundary, crash_at;
static bool fail_write, fail_install;

static void mutated(void) { if (++boundary == crash_at) longjmp(power_loss, 1); }
static int find(const char *path) {
    for (int i = 0; i < 8; ++i) if (files[i].exists && !strcmp(files[i].path, path)) return i;
    return -1;
}
static int create(const char *path) {
    int i = find(path);
    if (i >= 0) return i;
    for (i = 0; i < 8; ++i) if (!files[i].exists) {
        files[i].exists = true; files[i].size = 0; strcpy(files[i].path, path); return i;
    }
    assert(false); return -1;
}
static void seed(const char *suffix, const char *data) {
    char path[140]; snprintf(path, sizeof(path), "/3ds/PokeReader/time-lock/00040000001B5000.%s", suffix);
    int i = create(path); files[i].size = strlen(data); memcpy(files[i].data, data, files[i].size);
}
static void reset(bool corrupt) {
    memset(files, 0, sizeof(files)); boundary = crash_at = 0; fail_write = fail_install = false;
    seed("ini", corrupt ? "BAD" : "GOOD-old"); seed("bak", "GOOD-old");
}
static void assert_recoverable(void) {
    u8 data[512], out[512]; bool found = false;
    for (u32 backup = 0; backup <= 1; ++backup) {
        s32 count = host_tl_load(0x00040000001B5000ULL, backup, data, sizeof(data));
        if (count > 0 && tl_preset_edit(0, data, count, -1, 0, out, sizeof(out)) > 0) found = true;
    }
    assert(found);
}

Result fsInit(void) { return 0; }
void fsExit(void) {}
FS_Path fsMakePath(int type, const char *path) { (void)type; return (FS_Path){path}; }
Result FSUSER_OpenArchive(FS_Archive *a, int type, FS_Path p) { (void)type; (void)p; *a = 1; return 0; }
Result FSUSER_CloseArchive(FS_Archive a) { (void)a; return 0; }
Result FSUSER_CreateDirectory(FS_Archive a, FS_Path p, u32 flags) { (void)a; (void)p; (void)flags; return 0; }
Result FSUSER_OpenFile(Handle *h, FS_Archive a, FS_Path p, u32 flags, u32 attr) {
    (void)a; (void)attr; int i = find(p.text);
    if (i < 0 && (flags & FS_OPEN_CREATE)) i = create(p.text);
    if (i < 0) return -RS_NOTFOUND;
    *h = i + 1; return 0;
}
Result FSUSER_DeleteFile(FS_Archive a, FS_Path p) {
    (void)a; int i = find(p.text); if (i < 0) return -RS_NOTFOUND;
    files[i].exists = false; mutated(); return 0;
}
Result FSUSER_RenameFile(FS_Archive a, FS_Path from, FS_Archive b, FS_Path to) {
    (void)a; (void)b;
    if (fail_install && strstr(from.text, ".tmp")) return -1;
    int i = find(from.text); if (i < 0 || find(to.text) >= 0) return -1;
    strcpy(files[i].path, to.text); mutated(); return 0;
}
Result FSFILE_GetSize(Handle h, u64 *out) { *out = files[h - 1].size; return 0; }
Result FSFILE_Read(Handle h, u32 *out, u64 offset, void *data, u32 size) {
    assert(offset + size <= files[h - 1].size); memcpy(data, files[h - 1].data + offset, size); *out = size; return 0;
}
Result FSFILE_SetSize(Handle h, u64 size) { assert(size <= sizeof(files[0].data)); files[h - 1].size = size; mutated(); return 0; }
Result FSFILE_Write(Handle h, u32 *out, u64 offset, const void *data, u32 size, u32 flags) {
    (void)flags; *out = fail_write ? size / 2 : size;
    memcpy(files[h - 1].data + offset, data, *out); mutated(); return fail_write ? -1 : 0;
}
Result FSFILE_Close(Handle h) { (void)h; mutated(); return 0; }
Result svcCreateMutex(Handle *h, bool locked) { (void)locked; *h = 1; return 0; }
Result svcWaitSynchronization(Handle h, s64 timeout) { (void)h; (void)timeout; return 0; }
Result svcReleaseMutex(Handle h) { (void)h; return 0; }
u64 svcGetSystemTick(void) { return 1; }
u64 osGetTime(void) { return 0; }
Result svcConnectToPort(Handle *h, const char *name) { (void)h; (void)name; return -1; }
u32 *getThreadCommandBuffer(void) { static u32 buffer[8]; return buffer; }
u32 IPC_MakeHeader(u32 a, u32 b, u32 c) { return a + b + c; }
u32 IPC_Desc_Buffer(u32 a, u32 b) { return a + b; }
Result svcSendSyncRequest(Handle h) { (void)h; return 0; }
Result svcCloseHandle(Handle h) { (void)h; return 0; }

// Parser behavior is independently covered through the real Rust FFI tests.
s32 tl_preset_edit(u64 title, const u8 *in, u32 len, s32 field, s32 delta, u8 *out, u32 capacity) {
    (void)title; (void)field; (void)delta;
    if (len != 8 || memcmp(in, "GOOD-", 5) || capacity < len) return -1;
    memcpy(out, in, len); return len;
}

int main(void) {
    unsigned tested = 0;
    for (int corrupt = 0; corrupt <= 1; ++corrupt) {
        reset(corrupt);
        assert(host_tl_save(0x00040000001B5000ULL, (const u8 *)"GOOD-new", 8) == 0);
        unsigned boundaries = boundary;
        for (unsigned n = 1; n <= boundaries; ++n) {
            reset(corrupt); crash_at = n;
            if (setjmp(power_loss) == 0) host_tl_save(0x00040000001B5000ULL, (const u8 *)"GOOD-new", 8);
            crash_at = 0; assert_recoverable(); ++tested;
        }
    }
    reset(false); fail_write = true;
    assert(host_tl_save(0x00040000001B5000ULL, (const u8 *)"GOOD-new", 8) == -1);
    assert_recoverable();
    reset(false); fail_install = true;
    assert(host_tl_save(0x00040000001B5000ULL, (const u8 *)"GOOD-new", 8) == -1);
    assert_recoverable();
    printf("SD journal: %u interrupted boundaries and 2 IO failure cases passed.\n", tested);
    return 0;
}
