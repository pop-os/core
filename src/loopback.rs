use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Loopback {
    file: PathBuf,
    device: PathBuf,
    attached: bool,
}

impl Loopback {
    pub fn new<P: AsRef<Path>>(file: P) -> Result<Loopback> {
        log::debug!("Loopback::new {}", file.as_ref().display());

        let file = file.as_ref().canonicalize()?;

        let mut command = Command::new("losetup");
        command
            .arg("--partscan")
            .arg("--show")
            .arg("--find")
            .arg(&file);

        log::trace!("{:?}", command);
        let output = command.stdout(Stdio::piped()).spawn()?.wait_with_output()?;

        if output.status.success() {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let device = String::from_utf8(output.stdout).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("losetup output is not valid UTF-8: {}", err),
                )
            })?;

            Ok(Loopback {
                file: file,
                device: PathBuf::from(device.trim()),
                attached: true,
            })
        } else {
            Err(Error::new(
                ErrorKind::Other,
                format!("losetup attach failed with status: {}", output.status),
            ))
        }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn device(&self) -> &Path {
        &self.device
    }

    pub fn with<T, F: FnOnce(&mut Self) -> Result<T>>(mut self, function: F) -> Result<T> {
        if self.attached {
            let res = function(&mut self);
            self.detach()?;
            res
        } else {
            Err(Error::new(
                ErrorKind::Other,
                format!("loopback device not attached"),
            ))
        }
    }

    pub fn detach(&mut self) -> Result<()> {
        if self.attached {
            log::debug!("Loopback::detach {}", self.file.display());

            let mut command = Command::new("losetup");
            command.arg("--detach").arg(&self.device);

            log::trace!("{:?}", command);
            let status = command.status()?;
            if status.success() {
                self.attached = false;
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    format!("losetup detach failed with status: {}", status),
                ))
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for Loopback {
    fn drop(&mut self) {
        self.detach().expect("Loopback::drop");
    }
}
