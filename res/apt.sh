set -e

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

echo "Installing APT packages: $@"
apt-get install --yes "$@"

echo "Updating APT metadata again"
apt-get update

echo "Upgrading APT packages again"
apt-get upgrade --allow-downgrades --yes

echo "Automatically removing unused APT packages"
apt-get autoremove --purge --yes

echo "Removing temporary APT data"
apt-get clean
