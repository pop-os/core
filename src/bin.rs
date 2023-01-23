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

fn install(mount_dir: &Path, mount_efi_dir: &Path) -> io::Result<()> {
    log::info!("Resetting hostname");
    fs::write(
        mount_dir.join("etc/hostname"),
        include_bytes!("../res/etc/hostname"),
    )?;

    log::info!("Resetting Ubuntu APT repository");
    fs::write(
        mount_dir.join("etc/apt/sources.list"),
        include_bytes!("../res/etc/apt/sources.list"),
    )?;
    fs::write(
        mount_dir.join("etc/apt/sources.list.d/system.sources"),
        include_bytes!("../res/etc/apt/sources.list.d/system.sources"),
    )?;

    log::info!("Adding Pop!_OS APT repository");
    fs::write(
        mount_dir.join("etc/apt/sources.list.d/pop-os-release.sources"),
        include_bytes!("../res/etc/apt/sources.list.d/pop-os-release.sources"),
    )?;
    fs::write(
        mount_dir.join("etc/apt/trusted.gpg.d/pop-keyring-2017-archive.gpg"),
        include_bytes!("../res/etc/apt/trusted.gpg.d/pop-keyring-2017-archive.gpg"),
    )?;

    let kernelstub_dir = mount_dir.join("etc/kernelstub");
    if !kernelstub_dir.exists() {
        log::info!("Creating kernelstub configuration directory");
        fs::create_dir(&kernelstub_dir)?;
    }
    log::info!("Creating kernelstub configuration file");
    fs::write(
        kernelstub_dir.join("configuration"),
        include_bytes!("../res/etc/kernelstub/configuration"),
    )?;

    log::info!("Copying install script");
    fs::write(
        mount_dir.join("install.sh"),
        include_bytes!("../res/install.sh"),
    )?;

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

    log::info!("Running install script");
    Command::new("systemd-nspawn")
        .arg("--machine=pop-core-install")
        .arg("-D")
        .arg(&mount_dir)
        .arg("bash")
        .arg("/install.sh")
        .arg(&root_uuid)
        .arg(&efi_partuuid)
        .status()
        .and_then(check_status)?;

    log::info!("Removing install script");
    fs::remove_file(mount_dir.join("install.sh"))?;

    Ok(())
}

//TODO: use cache
pub fn bin() -> io::Result<()> {
    //TODO: ensure there are no active mounts inside any of the partial directories before removal!
    let mut cache = Cache::new("build", |name| ["debootstrap", "image"].contains(&name))?;

    let (debootstrap_dir, debootstrap_rebuilt) =
        cache.build("debootstrap", false, |partial_dir| {
            log::info!("Creating debootstrap");
            Debootstrap::new(&partial_dir).variant("minbase").run()?;
            Ok(())
        })?;

    let (image_dir, image_rebuilt) = cache.build("image", debootstrap_rebuilt, |partial_dir| {
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

        log::info!("Creating partition table");
        Command::new("parted")
            .arg("--script")
            .arg(&image_file)
            .arg("mklabel")
            .arg("gpt")
            .status()
            .and_then(check_status)?;

        log::info!("Adding EFI partition");
        Command::new("parted")
            .arg("--script")
            .arg(&image_file)
            .arg("mkpart")
            .arg("primary")
            .arg("fat32")
            .arg("0%")
            .arg("512")
            .status()
            .and_then(check_status)?;

        log::info!("Setting EFI partition as bootable");
        Command::new("parted")
            .arg("--script")
            .arg(&image_file)
            .arg("set")
            .arg("1")
            .arg("boot")
            .arg("on")
            .status()
            .and_then(check_status)?;

        log::info!("Adding root partition");
        Command::new("parted")
            .arg("--script")
            .arg(&image_file)
            .arg("mkpart")
            .arg("primary")
            .arg("btrfs")
            .arg("512")
            .arg("100%")
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
                log::info!("Copying debootstrap files");
                Command::new("cp")
                    .arg("--archive")
                    .arg("--no-target-directory")
                    .arg(&debootstrap_dir)
                    .arg(&mount_dir)
                    .status()
                    .and_then(check_status)?;

                let mount_efi_dir = mount_dir.join("boot/efi");
                if !mount_efi_dir.exists() {
                    log::info!("Creating EFI directory");
                    fs::create_dir(&mount_efi_dir)?;
                }

                log::info!("Mounting EFI directory");
                Mount::new(&part1_file, &mount_efi_dir, "vfat", 0, None)?
                    .with(|_mount_efi| install(&mount_dir, &mount_efi_dir))?;

                Ok(())
            })?;

            Ok(())
        })?;

        Ok(())
    })?;

    Ok(())
}