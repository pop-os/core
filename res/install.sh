set -e

PACKAGES=(
    kernelstub
    linux-system76
    pop-default-settings
    systemd
)

if [ "${HOSTNAME}" != "pop-core-install" ]
then
    echo "$0: must only be used for pop-core installation" >&2
    exit 1
fi

ROOT_UUID="$1"
EFI_PARTUUID="$2"
if [ -z "$ROOT_UUID" -o -z "${EFI_PARTUUID}" ]
then
    echo "$0 [root uuid] [efi partuuid]" >&2
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive
export LC_ALL=C

echo "Updating APT metadata"
apt-get update

echo "Upgrading APT packages"
apt-get upgrade --yes

echo "Installing APT packages: ${PACKAGES[@]}"
apt-get install --yes "${PACKAGES[@]}"

echo "Installing systemd-boot"
bootctl install --no-variables

#TODO: fix issues with ROOT_UUID not being found: kernelstub --manage-only --no-loader --verbose

echo "Copying loader files"
EFI_DIR="EFI/Pop_OS-${ROOT_UUID}"
mkdir -p "/boot/efi/${EFI_DIR}"
cp /boot/initrd.img "/boot/efi/${EFI_DIR}/initrd.img"
cp /boot/vmlinuz "/boot/efi/${EFI_DIR}/vmlinuz.efi"

echo "Setting up loader entry"
cat > /boot/efi/loader/entries/Pop_OS-current.conf <<EOF
title Pop!_OS
linux /${EFI_DIR}/vmlinuz.efi
initrd /${EFI_DIR}/initrd.img
options root=UUID=${ROOT_UUID} ro quiet loglevel=0 systemd.show_status=false splash
EOF

echo "Setting up loader configuration"
cat > /boot/efi/loader/loader.conf <<EOF
default Pop_OS-current
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
PARTUUID=${EFI_PARTUUID}  /boot/efi  vfat  umask=0077  0  0
UUID=${ROOT_UUID}  /  btrfs  defaults  0  1
EOF

#TODO: remove this!
echo "Setting root password to system76"
echo "root:system76" | chpasswd
