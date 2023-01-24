set -e

PACKAGES=(
    # Core
    btrfs-progs
    kernelstub
    linux-system76
    network-manager
    pop-default-settings
    # Desktop
    flatpak
    pop-gtk-theme
    pop-icon-theme
    pop-shop
    sway
    xwayland
)

if [ "${HOSTNAME}" != "pop-core-install" ]
then
    echo "$0: must only be used for pop-core installation" >&2
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive
export LC_ALL=C

echo "Updating APT metadata"
apt-get update

echo "Upgrading APT packages"
apt-get upgrade --yes

echo "Mark all APT packages as automatically installed"
manual="$(apt-mark showmanual)"
if [ -n "${manual}" ]
then
	apt-mark auto $manual
fi

echo "Installing APT packages: ${PACKAGES[@]}"
apt-get install --yes "${PACKAGES[@]}"

echo "Updating APT metadata again"
apt-get update

echo "Upgrading APT packages again"
apt-get upgrade --allow-downgrades --yes

echo "Automatically removing unused APT packages"
apt-get autoremove --purge --yes

echo "Removing temporary APT data"
apt-get clean

echo "Setting up NetworkManager"
mkdir -p /etc/NetworkManager/conf.d
touch /etc/NetworkManager/conf.d/10-globally-managed-devices.conf

echo "Creating user system76"
adduser \
    --quiet \
    --disabled-password \
    --shell /bin/bash \
    --home /home/system76 \
    --gecos System76 \
    system76

echo "Adding user system76 to adm group"
adduser system76 adm

echo "Adding user system76 to sudo group"
adduser system76 sudo

echo "Setting user system76 password to system76"
echo "system76:system76" | chpasswd
