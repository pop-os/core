use std::{
    fs, io,
    path::Path,
    process::{Command, Stdio},
    str,
};

use crate::{
    util::{check_output, check_status},
    Cache, Debootstrap, Loopback, Mount,
};

const SERVER_PACKAGES: &'static [&'static str] = &[
    "binutils", // for unified kernel image
    "btrfs-progs",
    "kernelstub",
    "linux-system76",
    "network-manager",
    "pop-default-settings",
    "shim-signed", // for secure boot
];

const DESKTOP_PACKAGES: &'static [&'static str] = &[
    "alacritty",
    "cosmic-session",
    "flatpak",
    "libegl1", // cosmic-comp dependency
    "libgl1-mesa-dri", // cosmic-comp dependency
    "libglib2.0-bin", // for gsettings command
    "pop-gtk-theme",
    "pop-icon-theme",
    "pop-wallpapers",
    "wireplumber",
];

fn server(partial_dir: &Path) -> io::Result<()> {
    log::info!("Resetting hostname");
    fs::write(
        partial_dir.join("etc/hostname"),
        include_bytes!("../res/etc/hostname"),
    )?;

    log::info!("Resetting Ubuntu APT repository");
    fs::write(
        partial_dir.join("etc/apt/sources.list"),
        include_bytes!("../res/etc/apt/sources.list"),
    )?;
    fs::write(
        partial_dir.join("etc/apt/sources.list.d/system.sources"),
        include_bytes!("../res/etc/apt/sources.list.d/system.sources"),
    )?;

    log::info!("Adding Pop!_OS APT repository");
    fs::write(
        partial_dir.join("etc/apt/sources.list.d/pop-os-release.sources"),
        include_bytes!("../res/etc/apt/sources.list.d/pop-os-release.sources"),
    )?;
    fs::write(
        partial_dir.join("etc/apt/trusted.gpg.d/pop-keyring-2017-archive.gpg"),
        include_bytes!("../res/etc/apt/trusted.gpg.d/pop-keyring-2017-archive.gpg"),
    )?;

    let kernelstub_dir = partial_dir.join("etc/kernelstub");
    if !kernelstub_dir.exists() {
        log::info!("Creating kernelstub configuration directory");
        fs::create_dir(&kernelstub_dir)?;
    }
    log::info!("Creating kernelstub configuration file");
    fs::write(
        kernelstub_dir.join("configuration"),
        include_bytes!("../res/etc/kernelstub/configuration"),
    )?;

    log::info!("Copying apt script");
    fs::write(
        partial_dir.join("apt.sh"),
        include_bytes!("../res/apt.sh"),
    )?;

    log::info!("Running apt script");
    Command::new("systemd-nspawn")
        .arg("--machine=pop-core-install")
        .arg("--resolv-conf=replace-host")
        .arg("-D")
        .arg(&partial_dir)
        .arg("bash")
        .arg("/apt.sh")
        .args(SERVER_PACKAGES)
        .status()
        .and_then(check_status)?;

    log::info!("Removing apt script");
    fs::remove_file(partial_dir.join("apt.sh"))?;

    Ok(())
}

fn desktop(partial_dir: &Path) -> io::Result<()> {
    log::info!("Copying apt script");
    fs::write(
        partial_dir.join("apt.sh"),
        include_bytes!("../res/apt.sh"),
    )?;

    log::info!("Running apt script");
    Command::new("systemd-nspawn")
        .arg("--machine=pop-core-install")
        .arg("--resolv-conf=replace-host")
        .arg("-D")
        .arg(&partial_dir)
        .arg("bash")
        .arg("/apt.sh")
        .args(SERVER_PACKAGES)
        .args(DESKTOP_PACKAGES)
        .status()
        .and_then(check_status)?;

    log::info!("Removing apt script");
    fs::remove_file(partial_dir.join("apt.sh"))?;

    Ok(())
}

fn image(mount_dir: &Path, mount_efi_dir: &Path) -> io::Result<()> {
    log::info!("Getting root UUID");
    let root_uuid = {
        let output = Command::new("findmnt")
            .arg("--noheadings")
            .arg("--output")
            .arg("UUID")
            .arg("--mountpoint")
            .arg(&mount_dir)
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .and_then(check_output)?;

        str::from_utf8(&output.stdout)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
            .trim()
            .to_string()
    };

    log::info!("Getting EFI PARTUUID");
    let efi_partuuid = {
        let output = Command::new("findmnt")
            .arg("--noheadings")
            .arg("--output")
            .arg("PARTUUID")
            .arg("--mountpoint")
            .arg(&mount_efi_dir)
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .and_then(check_output)?;

        str::from_utf8(&output.stdout)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
            .trim()
            .to_string()
    };

    log::info!("Copying image script");
    fs::write(
        mount_dir.join("image.sh"),
        include_bytes!("../res/image.sh"),
    )?;

    log::info!("Running image script");
    Command::new("systemd-nspawn")
        .arg("--machine=pop-core-install")
        .arg("-D")
        .arg(&mount_dir)
        .arg("bash")
        .arg("/image.sh")
        .arg(&root_uuid)
        .arg(&efi_partuuid)
        .status()
        .and_then(check_status)?;

    log::info!("Removing image script");
    fs::remove_file(mount_dir.join("image.sh"))?;

    Ok(())
}

//TODO: use cache
pub fn bin() -> io::Result<()> {
    //TODO: ensure there are no active mounts inside any of the partial directories before removal!
    let mut cache = Cache::new("build/cache", |name| {
        ["debootstrap", "desktop", "image", "server"].contains(&name)
    })?;

    let (debootstrap_dir, debootstrap_rebuilt) =
        cache.build("debootstrap", false, |partial_dir| {
            log::info!("Creating debootstrap");
            Debootstrap::new(&partial_dir).variant("minbase").run()?;
            Ok(())
        })?;

    let (server_dir, server_rebuilt) =
        cache.build("server", debootstrap_rebuilt, |partial_dir| {
            log::info!("Copying debootstrap files");
            Command::new("cp")
                .arg("--archive")
                .arg("--no-target-directory")
                .arg(&debootstrap_dir)
                .arg(&partial_dir)
                .status()
                .and_then(check_status)?;

            server(&partial_dir)
        })?;

    let (desktop_dir, desktop_rebuilt) =
        cache.build("desktop", server_rebuilt, |partial_dir| {
            log::info!("Copying server files");
            Command::new("cp")
                .arg("--archive")
                .arg("--no-target-directory")
                .arg(&server_dir)
                .arg(&partial_dir)
                .status()
                .and_then(check_status)?;

            desktop(&partial_dir)
        })?;

    let (image_dir, image_rebuilt) = cache.build("image", desktop_rebuilt, |partial_dir| {
        fs::create_dir(&partial_dir)?;

        //TODO: move logic to Rust as much as possible

        log::info!("Allocating image file");
        let image_file = partial_dir.join("image.raw");
        Command::new("fallocate")
            .arg("--length")
            .arg("8GiB")
            .arg("--posix")
            .arg(&image_file)
            .status()
            .and_then(check_status)?;

        log::info!("Partitioning image file");
        Command::new("sgdisk")
            .arg("--new=1:0:+512M")
            .arg("--typecode=1:0xef00")
            .arg("--new=2:0:0")
            .arg("--typecode=2:0x8304")
            .arg(&image_file)
            .status()
            .and_then(check_status)?;

        log::info!("Using loopback device");
        Loopback::new(&image_file)?.with(|loopback| {
            log::info!("Formatting EFI partition");
            //TODO: safer way of getting partition 1
            let part1_file = format!("{}p1", loopback.device().display());
            Command::new("mkfs.fat")
                .arg("-F")
                .arg("32")
                .arg(&part1_file)
                .status()
                .and_then(check_status)?;

            log::info!("Formatting BTRFS partition");
            //TODO: safer way of getting partition 2
            let part2_file = format!("{}p2", loopback.device().display());
            Command::new("mkfs.btrfs")
                .arg(&part2_file)
                .status()
                .and_then(check_status)?;

            log::info!("Mounting BTRFS partition");
            //TODO: use temporary directory?
            let mount_dir = partial_dir.join("mount");
            fs::create_dir(&mount_dir)?;
            Mount::new(&part2_file, &mount_dir, "btrfs", 0, None)?.with(|_mount| {
                for subvolume in &["home", "tmp", "var"] {
                    log::info!("Creating subvolume for /{}", subvolume);
                    Command::new("btrfs")
                        .arg("subvolume")
                        .arg("create")
                        .arg(mount_dir.join(subvolume))
                        .status()
                        .and_then(check_status)?;
                }

                log::info!("Copying desktop files");
                Command::new("cp")
                    .arg("--archive")
                    .arg("--no-target-directory")
                    .arg(&desktop_dir)
                    .arg(&mount_dir)
                    .status()
                    .and_then(check_status)?;

                let mount_efi_dir = mount_dir.join("efi");
                if !mount_efi_dir.exists() {
                    log::info!("Creating EFI directory");
                    fs::create_dir(&mount_efi_dir)?;
                }

                log::info!("Mounting EFI directory");
                Mount::new(&part1_file, &mount_efi_dir, "vfat", 0, None)?
                    .with(|_mount_efi| image(&mount_dir, &mount_efi_dir))?;

                log::info!("Make root read-only");
                Command::new("btrfs")
                    .arg("property")
                    .arg("set")
                    .arg("-ts")
                    .arg(&mount_dir)
                    .arg("ro")
                    .arg("true")
                    .status()
                    .and_then(check_status)?;

                Ok(())
            })?;

            Ok(())
        })?;

        Ok(())
    })?;

    Ok(())
}
