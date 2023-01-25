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
	kvm \
	    -bios build/qemu/firmware.rom \
	    -cpu host \
		-device ich9-intel-hda \
		-device hda-duplex \
		-device virtio-vga-gl \
		-display gtk,gl=on \
		-drive file=$<,format=raw,if=none,id=drive0 -device nvme,drive=drive0,serial=DRIVE0 \
		-m 4G \
		-machine q35 \
		-smp 4 \
		-vga none

systemd-nspawn: build/qemu/image.raw
	sudo systemd-nspawn --machine=pop-core --image=$<
