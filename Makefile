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

build/qemu/OVMF_CODE.fd:
	mkdir -p build/qemu
	cp /usr/share/OVMF/OVMF_CODE_4M.fd $@

build/qemu/OVMF_VARS.fd:
	mkdir -p build/qemu
	cp /usr/share/OVMF/OVMF_VARS_4M.fd $@

build/qemu/image.raw: build/cache/image/image.raw
	mkdir -p build/qemu
	cp $< $@

qemu: build/qemu/image.raw build/qemu/OVMF_CODE.fd build/qemu/OVMF_VARS.fd
	kvm \
	    -bios build/qemu/firmware.rom \
	    -cpu host \
		-device ich9-intel-hda \
		-device hda-duplex \
		-device virtio-vga-gl \
		-display gtk,gl=on \
		-drive file=$<,format=raw,if=none,id=drive0 -device nvme,drive=drive0,serial=DRIVE0 \
		-drive file=build/qemu/OVMF_CODE.fd,format=raw,if=pflash,readonly=on \
		-drive file=build/qemu/OVMF_VARS.fd,format=raw,if=pflash \
		-m 4G \
		-machine q35 \
		-smp 4 \
		-vga none

systemd-nspawn: build/qemu/image.raw
	sudo systemd-nspawn --machine=pop-core --image=$<
