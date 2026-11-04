ifeq ($(strip $(DEVKITARM)),)
$(error "DEVKITARM is required")
endif
include $(DEVKITARM)/3ds_rules

LIBREADER := ../reader_core/target/armv6k-nintendo-3ds/release/libpokereader.a
FLAGS := -march=armv6k -mtune=mpcore -mfloat-abi=hard -mtp=soft -Os -ffunction-sections -fdata-sections

.PHONY: all clean
all: build/PokeReader-Time.3dsx

build:
	mkdir -p build

build/main.o: source/main.c ../3gx/includes/time_lock.h | build
	$(CC) $(FLAGS) -I../3gx/includes -I$(CTRULIB)/include -c $< -o $@

build/io.o: ../3gx/sources/time_lock_io.c ../3gx/includes/time_lock.h | build
	$(CC) $(FLAGS) -I../3gx/includes -I$(CTRULIB)/include -c $< -o $@

build/PokeReader-Time.elf: build/main.o build/io.o $(LIBREADER)
	$(CXX) $(FLAGS) -specs=3dsx.specs -Wl,--gc-sections $^ -L$(CTRULIB)/lib -lctru -lm -o $@

build/PokeReader-Time.3dsx: build/PokeReader-Time.elf
	3dsxtool $< $@

clean:
	rm -rf build
