#include <3ds.h>
#include <stdio.h>
#include <string.h>
#include "time_lock.h"

static const u64 titles[] = {
    0x0004000000055D00ULL, 0x0004000000055E00ULL, 0x000400000011C400ULL, 0x000400000011C500ULL,
    0x0004000000164800ULL, 0x0004000000175E00ULL, 0x00040000001B5000ULL, 0x00040000001B5100ULL,
};
static const char *names[] = { "X", "Y", "Omega Ruby", "Alpha Sapphire", "Sun", "Moon", "Ultra Sun", "Ultra Moon" };
static const char *fields[] = { "Year", "Month", "Day", "Hour", "Minute", "Second", "Millisecond", "Request next boot", "Initial mode", "Reuse on full boot" };
static u8 preset[513], edited[513];

static s32 load(unsigned selected, const char **message)
{
    for (u32 backup = 0; backup <= 1; ++backup) {
        s32 count = host_tl_load(titles[selected], backup, edited, 512);
        if (count > 0) {
            s32 valid = tl_preset_edit(titles[selected], edited, count, -1, 0, preset, 512);
            if (valid > 0) { *message = backup ? "Backup recovered; A saves repair" : "Preset loaded"; return valid; }
        }
    }
    *message = "No valid preset; disabled defaults";
    return tl_preset_default(titles[selected], host_tl_real_ms(), preset, 512);
}

int main(void)
{
    gfxInitDefault();
    // The editor is a normal 2D top-screen app. Explicitly reset these modes
    // so it cannot inherit a wide/3D framebuffer layout on some firmware.
    gfxSetWide(false);
    gfxSet3D(false);
    PrintConsole top_screen;
    consoleInit(GFX_TOP, &top_screen);
    // consoleInit uses a single visible buffer. The editor redraws every
    // frame, so use the normal double-buffer path and refresh the console's
    // draw pointer after each swap to avoid showing a partially cleared frame.
    gfxSetDoubleBuffering(GFX_TOP, true);
    // Keep a known 1-based window with a margin and a standard-width layout.
    // This avoids clipped first columns when the runtime console state differs.
    consoleSetWindow(&top_screen, 2, 1, 46, 28);
    consoleSelect(&top_screen);
    if (R_FAILED(host_tl_platform_init())) { gfxExit(); return 1; }
    // Start with Pokemon X so the first hardware test matches the current
    // validation target; Right switches to Y and the other supported titles.
    unsigned selected = 0, field = 0;
    const char *message = "";
    s32 length = load(selected, &message);
    bool dirty = false;
    while (aptMainLoop()) {
        hidScanInput();
        u32 keys = hidKeysDown();
        if (keys & KEY_START) break;
        if (keys & (KEY_LEFT | KEY_RIGHT)) {
            // Require saving or explicit B discard before switching a dirty title.
            if (dirty) message = "A saves, B discards before changing game";
            else {
                selected = (selected + ((keys & KEY_RIGHT) ? 1 : 7)) % 8;
                length = load(selected, &message);
            }
        } else if (keys & KEY_B) { length = load(selected, &message); dirty = false; }
        else if (keys & KEY_L) field = (field + 9) % 10;
        else if (keys & KEY_R) field = (field + 1) % 10;
        else if ((keys & (KEY_UP | KEY_DOWN)) && length > 0) {
            s32 count = tl_preset_edit(titles[selected], preset, length, field, (keys & KEY_UP) ? 1 : -1, edited, 512);
            if (count > 0) { memcpy(preset, edited, count); length = count; dirty = true; message = "Edited; A saves"; }
            else message = "Invalid date/range; unchanged";
        } else if ((keys & KEY_A) && length > 0) {
            if (host_tl_save(titles[selected], preset, length) == 0) { dirty = false; message = "Saved. Clock adapter is unverified."; }
            else message = "Save failed. Previous preset retained.";
        }
        top_screen.frameBuffer = (u16 *)gfxGetFramebuffer(GFX_TOP, GFX_LEFT, NULL, NULL);
        consoleSelect(&top_screen);
        consoleClear();
        printf("PokeReader Time - DEV\n\n");
        printf("Game: %s%s\n", names[selected], dirty ? " *" : "");
        printf("Clock adapter: UNVERIFIED\nNo system-clock changes.\n\n");
        if (length > 0) { preset[length] = 0; printf("%s\n", preset); }
        printf("Field: %s\n\n", fields[field]);
        printf("Left/Right: game  L/R: field\nUp/Down: edit     A: save\nB: reload         START: exit\n\n%s\n", message);
        gfxFlushBuffers();
        gfxSwapBuffers();
        gspWaitForVBlank();
    }
    gfxExit();
    return 0;
}
