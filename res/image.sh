set -e

if [ "${HOSTNAME}" != "pop-core-install" ]
then
    echo "$0: must only be used for pop-core installation" >&2
    exit 1
fi

ROOT_UUID="$1"
EFI_PARTUUID="$2"
if [ -z "${ROOT_UUID}" -o -z "${EFI_PARTUUID}" ]
then
    echo "$0 [root uuid] [efi partuuid]" >&2
    exit 1
fi

export LC_ALL=C

######## USER SETUP ########

USER=pop

echo "Creating user ${USER}"
adduser \
    --quiet \
    --disabled-password \
    --shell /bin/bash \
    --home "/home/${USER}" \
    --gecos "${USER}" \
    "${USER}"

echo "Adding user ${USER} to adm group"
adduser "${USER}" adm

echo "Adding user ${USER} to sudo group"
adduser "${USER}" sudo

echo "Setting user ${USER} password to ${USER}"
echo "${USER}:${USER}" | chpasswd

######## BOOTLOADER SETUP ########

CMDLINE="root=UUID=${ROOT_UUID} ro"

TEMPDIR="$(mktemp --directory)"
pushd "${TEMPDIR}"

echo "Creating machine owner key"
openssl req \
    -newkey rsa:4096 \
    -nodes \
    -keyout /etc/kernelstub/mok.key \
    -new \
    -x509 \
    -sha256 \
    -days 3650 \
    -subj "/CN=Machine Owner Key/" \
    -out /etc/kernelstub/mok.crt

echo "Copy shim to EFI boot directory"
mkdir /boot/efi/EFI
mkdir /boot/efi/EFI/BOOT
cp /usr/lib/shim/shimx64.efi.signed /boot/efi/EFI/BOOT/BOOTX64.EFI
cp /usr/lib/shim/mmx64.efi /boot/efi/EFI/BOOT/mmx64.efi

echo "Adding SBAT to systemd-boot"
cp /usr/lib/systemd/boot/efi/systemd-bootx64.efi systemd-bootx64.unsigned.efi
SYSTEMD_VERSION="$(dpkg-query -Wf '${Version}' systemd)"
cat > sbat.csv <<EOF
sbat,1,SBAT Version,sbat,1,https://github.com/rhboot/shim/blob/main/SBAT.md
systemd.pop-os,1,Pop!_OS,systemd,${SYSTEMD_VERSION},https://github.com/pop-os/systemd
EOF
objcopy \
    --add-section .sbat=sbat.csv \
    --change-section-vma .sbat=0x10000000 \
    systemd-bootx64.unsigned.efi

echo "Signing systemd-boot with machine owner key and copying to grubx64.efi"
sbsign \
    --key /etc/kernelstub/mok.key \
    --cert /etc/kernelstub/mok.crt \
    --output /boot/efi/EFI/BOOT/grubx64.efi \
    systemd-bootx64.unsigned.efi

echo

#TODO: fix issues with ROOT_UUID not being found: kernelstub --manage-only --no-loader --verbose

echo "Creating EFI directory"
EFI_DIR="EFI/Pop_OS-${ROOT_UUID}"
mkdir "/boot/efi/${EFI_DIR}"

echo "Creating mok.cer for enrollment"
openssl x509 -outform DER -in /etc/kernelstub/mok.crt -out "/boot/efi/${EFI_DIR}/mok.cer"
cp "/boot/efi/${EFI_DIR}/mok.cer" "/boot/efi/MOK-Pop_OS-${ROOT_UUID}.cer"

echo "Creating unified kernel"
echo -n "${CMDLINE}" > cmdline
objcopy \
    --add-section .osrel=/usr/lib/os-release --change-section-vma .osrel=0x20000 \
    --add-section .cmdline=cmdline --change-section-vma .cmdline=0x30000 \
    --add-section .linux=/boot/vmlinuz --change-section-vma .linux=0x2000000 \
    --add-section .initrd=/boot/initrd.img --change-section-vma .initrd=0x3000000 \
    /usr/lib/systemd/boot/efi/linuxx64.efi.stub \
    vmlinuz.unsigned.efi

echo "Signing unified kernel"
sbsign \
    --key /etc/kernelstub/mok.key \
    --cert /etc/kernelstub/mok.crt \
    --output "/boot/efi/${EFI_DIR}/vmlinuz.efi" \
    vmlinuz.unsigned.efi

echo "Setting up loader configuration"
mkdir /boot/efi/loader
cat > /boot/efi/loader/loader.conf <<EOF
default Pop_OS-current
timeout 60
EOF

echo "Setting up loader entry"
mkdir /boot/efi/loader/entries
cat > /boot/efi/loader/entries/Pop_OS-current.conf <<EOF
title Pop!_OS
efi /${EFI_DIR}/vmlinuz.efi
EOF

cat > /boot/efi/loader/entries/Pop_OS-old.conf <<EOF
title Pop!_OS (@root.old)
efi /${EFI_DIR}/vmlinuz.efi
options ${CMDLINE} rootflags=subvol=@root.old
EOF

cat > /boot/efi/loader/entries/Pop_OS-original.conf <<EOF
title Pop!_OS (@root.original)
efi /${EFI_DIR}/vmlinuz.efi
options ${CMDLINE} rootflags=subvol=@root.original
EOF

echo "Setting up fstab"
cat > /etc/fstab <<EOF
# /etc/fstab: static file system information.
#
# Use 'blkid' to print the universally unique identifier for a
# device; this may be used with UUID= as a more robust way to name devices
# that works even if disks are added and removed. See fstab(5).
#
# <file system>  <mount point>  <type>  <options>  <dump>  <pass>
#
# NOTE: / is automatically mounted and does not require an entry
PARTUUID=${EFI_PARTUUID}  /boot/efi  vfat  umask=0077  0  0
UUID=${ROOT_UUID}  /home  btrfs  defaults,subvol=@home  0  0
UUID=${ROOT_UUID}  /tmp  btrfs  defaults,subvol=@tmp  0  0
UUID=${ROOT_UUID}  /var  btrfs  defaults,subvol=@var  0  0
EOF

echo "Enabling kernelstub"
sed -i 's/"live_mode": true,/"live_mode": false,/' /etc/kernelstub/configuration

popd
rm -rf "${TEMPDIR}"

######## MISC SETUP #######

echo "Setting up NetworkManager"
mkdir -p /etc/NetworkManager/conf.d
touch /etc/NetworkManager/conf.d/10-globally-managed-devices.conf

echo "Setting up systemd-resolved"
ln -sf ../run/systemd/resolve/stub-resolv.conf /etc/resolv.conf

echo "Setting up terminal hotkey"
sed -i 's/gnome-terminal/alacritty/g' /etc/cosmic-comp/config.ron

echo "Making pop-core executable"
chmod +x /usr/bin/pop-core

RELOCATE=(
    "/media:/var/media"
    "/mnt:/var/mnt"
    "/opt:/var/opt"
    "/root:/home/root"
    "/srv:/var/srv"
    "/usr/local:/var/usr_local"
    "/var/lib/apt:/usr/var_lib_apt"
    "/var/lib/dpkg:/usr/var_lib_dpkg"
)
for config in "${RELOCATE[@]}"
do
    source="${config%:*}"
    dest="${config##*:}"
    echo "Relocating ${source} to ${dest}"
    mv --no-clobber --no-target-directory "${source}" "${dest}"
    ln --relative --symbolic "${dest}" "${source}"
done
