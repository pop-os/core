SRC=\
	Cargo.lock \
	Cargo.toml \
	$(shell find res -type f) \
	$(shell find src -type f)

all: build/cache/image/image.raw

target/release/pop-core: $(SRC)
	cargo build --release

build/cache/image/image.raw: target/release/pop-core
	mkdir -p build/cache
	sudo $<

build/qemu/firmware.rom:
	mkdir -p build/qemu
	cp /usr/share/OVMF/OVMF_CODE.fd $@

build/qemu/image.raw: build/cache/image/image.raw
	mkdir -p build/qemu
	cp $< $@

qemu: build/qemu/image.raw build/qemu/firmware.rom
	#TODO: use virtio GPU?
	kvm \
	    -bios build/qemu/firmware.rom \
	    -cpu host \
	    -hda $< \
	    -m 4G \
	    -smp 4 \
	    -vga qxl
