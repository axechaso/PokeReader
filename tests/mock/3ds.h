#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
typedef uint8_t u8;
typedef uint32_t u32;
typedef uint64_t u64;
typedef int32_t s32;
typedef int64_t s64;
typedef s32 Result;
typedef u32 Handle;
typedef u64 FS_Archive;
typedef struct { const char *text; } FS_Path;
#define U64_MAX UINT64_MAX
#define R_FAILED(r) ((r) < 0)
#define R_SUCCEEDED(r) ((r) >= 0)
#define R_SUMMARY(r) (-(r))
#define RS_NOTFOUND 2
#define ARCHIVE_SDMC 1
#define PATH_EMPTY 0
#define PATH_ASCII 1
#define FS_OPEN_READ 1
#define FS_OPEN_WRITE 2
#define FS_OPEN_CREATE 4
#define FS_WRITE_FLUSH 1
#define IPC_BUFFER_R 1
Result fsInit(void);
void fsExit(void);
FS_Path fsMakePath(int type, const char *path);
Result FSUSER_OpenArchive(FS_Archive *, int, FS_Path);
Result FSUSER_CloseArchive(FS_Archive);
Result FSUSER_OpenFile(Handle *, FS_Archive, FS_Path, u32, u32);
Result FSUSER_CreateDirectory(FS_Archive, FS_Path, u32);
Result FSUSER_DeleteFile(FS_Archive, FS_Path);
Result FSUSER_RenameFile(FS_Archive, FS_Path, FS_Archive, FS_Path);
Result FSFILE_GetSize(Handle, u64 *);
Result FSFILE_Read(Handle, u32 *, u64, void *, u32);
Result FSFILE_SetSize(Handle, u64);
Result FSFILE_Write(Handle, u32 *, u64, const void *, u32, u32);
Result FSFILE_Close(Handle);
Result svcCreateMutex(Handle *, bool);
Result svcWaitSynchronization(Handle, s64);
Result svcReleaseMutex(Handle);
u64 svcGetSystemTick(void);
u64 osGetTime(void);
Result svcConnectToPort(Handle *, const char *);
u32 *getThreadCommandBuffer(void);
u32 IPC_MakeHeader(u32, u32, u32);
u32 IPC_Desc_Buffer(u32, u32);
Result svcSendSyncRequest(Handle);
Result svcCloseHandle(Handle);
