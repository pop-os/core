use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug)]
pub struct Debootstrap {
    suite: String,
    target: PathBuf,
    mirror: String,
    arch: String,
    include: Vec<String>,
    exclude: Vec<String>,
    variant: Option<String>,
}

impl Debootstrap {
    pub fn new<P: AsRef<Path>>(target: P) -> Self {
        Self {
            suite: "jammy".to_string(),
            target: target.as_ref().to_owned(),
            arch: "amd64".to_string(),
            mirror: "https://apt.pop-os.org/ubuntu".to_string(),
            include: Vec::new(),
            exclude: Vec::new(),
            variant: None,
        }
    }

    //TODO: builder functions to set each argument

    pub fn include_package(mut self, package: impl Into<String>) -> Self {
        //TODO: ensure there are no commas in package?
        self.include.push(package.into());
        self
    }

    pub fn exclude_package(mut self, package: impl Into<String>) -> Self {
        //TODO: ensure there are no commas in package?
        self.exclude.push(package.into());
        self
    }

    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
        self
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new("debootstrap");
        if !self.include.is_empty() {
            command.arg(&format!("--include={}", self.include.join(",")));
        }
        if !self.exclude.is_empty() {
            command.arg(&format!("--exclude={}", self.exclude.join(",")));
        }
        if let Some(variant) = &self.variant {
            command.arg(&format!("--variant={}", variant));
        }
        command
            .arg(&format!("--arch={}", self.arch))
            .arg(&self.suite)
            .arg(&self.target)
            .arg(&self.mirror);
        command
    }

    pub fn run(&self) -> io::Result<()> {
        let status = self.command().status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("debootstrap exited with {}", status),
            ))
        }
    }
}
