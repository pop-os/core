set -e

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

export LC_ALL=C

######## BOOTLOADER SETUP ########

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

echo "Creating pop-core-autologin binary"
cat > /usr/bin/pop-core-autologin <<EOF
#!/usr/bin/env bash

set -ex

gsettings set org.gnome.desktop.interface color-scheme prefer-dark
gsettings set org.gnome.desktop.interface cursor-theme Pop
gsettings set org.gnome.desktop.interface gtk-theme Pop-dark
gsettings set org.gnome.desktop.interface icon-theme Pop

exec start-cosmic
EOF

echo "Setting pop-core-autologin binary executable"
chmod +x /usr/bin/pop-core-autologin

echo "Creating pop-core-autologin service"
cat > /usr/lib/systemd/system/pop-core-autologin.service <<EOF
[Unit]
Description=pop-core-autologin
OnFailure=getty@tty1.service
Conflicts=getty@tty1.service
After=graphical.target

[Service]
User=${USER}
ExecStart=/usr/bin/pop-core-autologin
WorkingDirectory=/home/${USER}
PAMName=login
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=yes

[Install]
WantedBy=graphical.target
EOF

echo "Enabling pop-core-autologin service"
mkdir -p /etc/systemd/system/graphical.target.wants
ln -s /usr/lib/systemd/system/pop-core-autologin.service /etc/systemd/system/graphical.target.wants/pop-core-autologin.service

echo "Relocating folders"
mv /media /var/media
ln -s var/media /media

mv /mnt /var/mnt
ln -s var/mnt /mnt

mv /opt /var/opt
ln -s var/opt /opt

mv /root /home/root
ln -s home/root /root

mv /srv /var/srv
ln -s var/srv /srv

mv /usr/local /var/usrlocal
ln -s var/usrlocal /usr/local
