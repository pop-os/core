#!/usr/bin/env bash

set -ex

mkdir -p build

if [ ! -d build/qemu ]
then
    cargo fmt
    cargo build --release
    time sudo target/release/pop-core

    mkdir build/qemu
    cp build/cache/image/image.raw build/qemu/image.raw
    cp /usr/share/OVMF/OVMF_CODE.fd build/qemu/firmware.rom
fi

kvm \
    -bios build/qemu/firmware.rom \
    -cpu host \
    -hda build/qemu/image.raw \
    -m 4G \
    -smp 4 \
    -vga virtio
