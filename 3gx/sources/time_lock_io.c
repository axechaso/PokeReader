#include "time_lock.h"
#include <stdio.h>
#include <string.h>

#define TL_DIR "/3ds/PokeReader/time-lock"
#define TL_MAX_PRESET 512

static Handle state_mutex;

Result host_tl_platform_init(void) { return svcCreateMutex(&state_mutex, false); }
u64 host_tl_tick(void) { return svcGetSystemTick(); }
s64 host_tl_real_ms(void) { return (s64)osGetTime(); }
void host_tl_lock(void) { svcWaitSynchronization(state_mutex, U64_MAX); }
void host_tl_unlock(void) { svcReleaseMutex(state_mutex); }

static Result open_sd(FS_Archive *archive)
{
    Result result = fsInit();
    if (R_FAILED(result)) return result;
    result = FSUSER_OpenArchive(archive, ARCHIVE_SDMC, fsMakePath(PATH_EMPTY, ""));
    if (R_FAILED(result)) fsExit();
    return result;
}

static void close_sd(FS_Archive archive)
{
    FSUSER_CloseArchive(archive);
    fsExit();
}

static void preset_path(char *out, size_t size, u64 title, const char *suffix)
{
    snprintf(out, size, TL_DIR "/%016llX.%s", (unsigned long long)title, suffix);
}

// Returns 0 for absent, -1 for errors (including empty/oversized files).
static s32 read_file(FS_Archive archive, const char *path, u8 *out, u32 capacity)
{
    Handle file;
    Result result = FSUSER_OpenFile(&file, archive, fsMakePath(PATH_ASCII, path), FS_OPEN_READ, 0);
    if (R_FAILED(result)) return R_SUMMARY(result) == RS_NOTFOUND ? 0 : -1;
    u64 size = 0;
    u32 count = 0;
    result = FSFILE_GetSize(file, &size);
    if (R_SUCCEEDED(result) && size > 0 && size <= capacity)
        result = FSFILE_Read(file, &count, 0, out, (u32)size);
    else result = -1;
    Result closed = FSFILE_Close(file);
    return R_SUCCEEDED(result) && R_SUCCEEDED(closed) && count == size ? (s32)count : -1;
}

static Result write_file(FS_Archive archive, const char *path, const u8 *bytes, u32 length)
{
    Handle file;
    Result result = FSUSER_OpenFile(&file, archive, fsMakePath(PATH_ASCII, path), FS_OPEN_CREATE | FS_OPEN_WRITE, 0);
    if (R_FAILED(result)) return result;
    result = FSFILE_SetSize(file, length);
    u32 count = 0;
    if (R_SUCCEEDED(result)) result = FSFILE_Write(file, &count, 0, bytes, length, FS_WRITE_FLUSH);
    Result closed = FSFILE_Close(file);
    if (R_FAILED(result)) return result;
    return count == length ? closed : -1;
}

static Result move_file(FS_Archive archive, const char *from, const char *to)
{
    return FSUSER_RenameFile(archive, fsMakePath(PATH_ASCII, from), archive, fsMakePath(PATH_ASCII, to));
}

static void make_directories(FS_Archive archive)
{
    const char *paths[] = { "/3ds", "/3ds/PokeReader", TL_DIR, TL_DIR "/logs" };
    for (u32 i = 0; i < sizeof(paths) / sizeof(paths[0]); ++i)
        FSUSER_CreateDirectory(archive, fsMakePath(PATH_ASCII, paths[i]), 0);
}

s32 host_tl_load(u64 title, u32 backup, u8 *out, u32 capacity)
{
    if (!out || capacity > TL_MAX_PRESET || backup > 1) return -1;
    FS_Archive archive;
    if (R_FAILED(open_sd(&archive))) return -1;
    char path[100];
    preset_path(path, sizeof(path), title, backup ? "bak" : "ini");
    s32 count = read_file(archive, path, out, capacity);
    close_sd(archive);
    return count;
}

s32 host_tl_save(u64 title, const u8 *bytes, u32 length)
{
    u8 verify[TL_MAX_PRESET];
    // The shared Rust parser rejects wrong titles, unknown policies and malformed input.
    if (!bytes || !length || length > TL_MAX_PRESET ||
        tl_preset_edit(title, bytes, length, -1, 0, verify, sizeof(verify)) <= 0) return -1;
    FS_Archive archive;
    if (R_FAILED(open_sd(&archive))) return -1;
    make_directories(archive);
    char primary[100], temporary[100], backup[100];
    preset_path(primary, sizeof(primary), title, "ini");
    preset_path(temporary, sizeof(temporary), title, "tmp");
    preset_path(backup, sizeof(backup), title, "bak");
    Result result = write_file(archive, temporary, bytes, length);
    if (R_SUCCEEDED(result)) {
        s32 count = read_file(archive, temporary, verify, sizeof(verify));
        if (count != (s32)length || memcmp(bytes, verify, length) != 0) result = -1;
    }
    bool moved_primary = false;
    if (R_SUCCEEDED(result)) {
        u8 canonical[TL_MAX_PRESET];
        s32 count = read_file(archive, primary, verify, sizeof(verify));
        if (count > 0 && tl_preset_edit(title, verify, count, -1, 0, canonical, sizeof(canonical)) > 0) {
            // At every rename boundary there is a fully validated primary or backup.
            FSUSER_DeleteFile(archive, fsMakePath(PATH_ASCII, backup));
            result = move_file(archive, primary, backup);
            moved_primary = R_SUCCEEDED(result);
        } else if (count != 0) {
            // Never overwrite a good backup with a corrupt primary.
            result = FSUSER_DeleteFile(archive, fsMakePath(PATH_ASCII, primary));
        }
    }
    if (R_SUCCEEDED(result)) result = move_file(archive, temporary, primary);
    if (R_FAILED(result) && moved_primary) move_file(archive, backup, primary);
    close_sd(archive);
    return R_SUCCEEDED(result) ? 0 : -1;
}

s32 host_tl_export(u64 title, u64 session, const u8 *bytes, u32 length)
{
    if (!bytes || length > 16384) return -1;
    FS_Archive archive;
    if (R_FAILED(open_sd(&archive))) return -1;
    make_directories(archive);
    char path[140];
    snprintf(path, sizeof(path), TL_DIR "/logs/%016llX-%016llX.csv",
        (unsigned long long)title, (unsigned long long)session);
    Result result = write_file(archive, path, bytes, length);
    close_sd(archive);
    return R_SUCCEEDED(result) ? 0 : -1;
}

// Luma's documented plugin-loader IPC, also works before overlay hooks exist.
void host_tl_message(const char *body)
{
    Handle service;
    if (R_FAILED(svcConnectToPort(&service, "plg:ldr"))) return;
    const char *title = "PokeReader development build";
    u32 *command = getThreadCommandBuffer();
    command[0] = IPC_MakeHeader(6, 0, 4);
    command[1] = IPC_Desc_Buffer(strlen(title), IPC_BUFFER_R);
    command[2] = (u32)title;
    command[3] = IPC_Desc_Buffer(strlen(body), IPC_BUFFER_R);
    command[4] = (u32)body;
    svcSendSyncRequest(service);
    svcCloseHandle(service);
}
