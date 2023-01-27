use std::{
    fs, io,
    path::Path,
    process::{Command, Stdio},
    str,
};

use crate::{
    util::{check_output, check_status},
    Mount,
};

fn btrfs_subvolid<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let output = Command::new("btrfs")
        .arg("inspect-internal")
        .arg("rootid")
        .arg(path.as_ref())
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .and_then(check_output)?;

    str::from_utf8(&output.stdout)
        .map(|x| x.trim().to_string())
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

//TODO: could use atomic swaps (renameat2?)
fn run_with_top_dir(top_dir: &Path, command: &String, args: &Vec<String>) -> io::Result<()> {
    log::info!("Getting root subvolid");
    let root_subvolid = btrfs_subvolid("/")?;

    log::info!("Getting hostname");
    let hostname = fs::read_to_string("/etc/hostname")?.trim().to_string();

    //TODO: use generation
    let root = top_dir.join("@root");
    let root_new = top_dir.join("@root.new");
    let root_old = top_dir.join("@root.old");

    if root_new.exists() {
        if btrfs_subvolid(&root_new)? == root_subvolid {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Booted root somehow at @root.new",
            ));
        } else {
            log::info!("Deleting @root.new");
            Command::new("btrfs")
                .arg("subvolume")
                .arg("delete")
                .arg(&root_new)
                .status()
                .and_then(check_status)?;
        }
    }

    log::info!("Creating writable snapshot of @root named @root.new");
    Command::new("btrfs")
        .arg("subvolume")
        .arg("snapshot")
        .arg(&root)
        .arg(&root_new)
        .status()
        .and_then(check_status)?;

    //TODO: capture result
    log::info!("Running command in container");
    Command::new("systemd-nspawn")
        .arg("--bind-ro=/home")
        //TODO: should /var be snapshotted or readonly?
        .arg("--bind=/var")
        .arg(&format!("--directory={}", root_new.display()))
        .arg(&format!("--machine={}", &hostname))
        .arg("--quiet")
        //TODO: fix bad /etc/resolv.conf!
        .arg("--resolv-conf=replace-host")
        .arg("--")
        .arg(command)
        .args(args)
        .status()
        .and_then(check_status)?;

    log::info!("Setting @root.new as read-only");
    Command::new("btrfs")
        .arg("property")
        .arg("set")
        .arg("-t")
        .arg("subvol")
        .arg(&root_new)
        .arg("ro")
        .arg("true")
        .status()
        .and_then(check_status)?;

    log::info!("Setting / as default subvolume");
    Command::new("btrfs")
        .arg("subvolume")
        .arg("set-default")
        .arg("/")
        .status()
        .and_then(check_status)?;

    log::info!("Saving booted root as @root.old");
    {
        if root_old.exists() {
            if btrfs_subvolid(&root_old)? == root_subvolid {
                log::info!("Booted root already saved as @root.old");
            } else {
                log::info!("Deleting @root.old");
                Command::new("btrfs")
                    .arg("subvolume")
                    .arg("delete")
                    .arg(&root_old)
                    .status()
                    .and_then(check_status)?;
            }
        }

        if !root_old.exists() {
            if btrfs_subvolid(&root)? == root_subvolid {
                log::info!("Moving @root to @root.old");
                fs::rename(&root, &root_old)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Booted root not found at @root",
                ));
            }
        }
    }

    log::info!("Saving @root.new as @root");
    {
        if root.exists() {
            if btrfs_subvolid(&root)? == root_subvolid {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Booted root still at @root",
                ));
            } else {
                log::info!("Deleting @root");
                Command::new("btrfs")
                    .arg("subvolume")
                    .arg("delete")
                    .arg(&root)
                    .status()
                    .and_then(check_status)?;
            }
        }

        log::info!("Moving @root.new to @root");
        fs::rename(&root_new, &root)?;
    }

    log::info!("Setting @root as default subvolume");
    Command::new("btrfs")
        .arg("subvolume")
        .arg("set-default")
        .arg(&root)
        .status()
        .and_then(check_status)?;

    Ok(())
}

pub fn run(command: String, args: Vec<String>) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "must be run as root",
        ));
    }

    //TODO: get root uuid without an external command
    log::info!("Getting root UUID");
    let root_uuid = {
        let output = Command::new("findmnt")
            .arg("--noheadings")
            .arg("--output")
            .arg("UUID")
            .arg("--mountpoint")
            .arg("/")
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .and_then(check_output)?;

        str::from_utf8(&output.stdout)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
            .trim()
            .to_string()
    };

    let top_dir = Path::new("/tmp/pop-core-change");
    if top_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "/tmp/pop-core-change already exists, is pop-core already running?",
        ));
    }

    // This atomically ensures only one pop-core is doing changes at a time
    log::info!("Creating temporary directory");
    fs::create_dir(&top_dir)?;

    log::info!("Mounting btrfs top level");
    let mut mount = Mount::new(
        &Path::new("/dev/disk/by-uuid").join(root_uuid),
        &top_dir,
        "btrfs",
        0,
        Some("subvol=/"),
    )?;

    let res = run_with_top_dir(&top_dir, &command, &args);

    log::info!("Unmounting btrfs top level");
    match mount.unmount(false) {
        Ok(()) => {
            log::info!("Removing temporary directory");
            fs::remove_dir(&top_dir)?;
        }
        Err(err) => {
            log::error!("Failed to unmount btrfs top level: {}", err);
        }
    }

    res
}
