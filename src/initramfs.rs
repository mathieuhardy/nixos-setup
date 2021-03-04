use clap;
use std::fs;
use std::io::Write;
use std::path;
use std::process;
use std::thread;
use std::time;

use super::env;
use super::filesystem;
use super::error;
use super::lvm;
use super::partition;
use super::traits::{CliCommand, Validate};

const ARG_HOST: &str = "host";
const ARG_PASSWORD: &str = "password";

/// Command structure for creating initramfs on generated filesystem
#[derive(Debug)]
pub struct Command {
    host: String,
    password: String,
    key_file: String,
    key_filename: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return !self.host.is_empty() &&
            !self.key_file.is_empty() &&
            !self.key_filename.is_empty();
    }
}

impl CliCommand for Command {
    fn name(&self) -> &'static str {
        return "initramfs";
    }

    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Create initramfs")
            .version(version)
            .author(author)
            // Host argument
            .arg(clap::Arg::with_name(ARG_HOST)
                .long(ARG_HOST)
                .help("Host name (optional if a .env file is present)")
                .takes_value(true))
            // Password argument
            .arg(clap::Arg::with_name(ARG_PASSWORD)
                .long(ARG_PASSWORD)
                .help("Password used to decrypt filesystems")
                .takes_value(true));
    }

    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_HOST => {
                    self.host = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_HOST),
                    };
                },

                &ARG_PASSWORD => {
                    self.password = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_PASSWORD),
                    };
                },

                _ => {
                    return inval_error!(arg.0);
                }
            }
        }

        if !self.is_valid() {
            self.fill_with_env()?;
        }

        log::info!("{:#?}", self);

        if !self.is_valid() {
            return generic_error!("Invalid configuration");
        }

        // Create root
        let root = path::Path::new("/").join("mnt").join("root");

        match fs::create_dir_all(&root) {
            Ok(_) => log::info!("`{:?}` created", &root),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Create efi path
        let efi = root.join("boot").join("efi");

        // Create initramfs path
        let initramfs = root.join("boot").join("initrd.keys.gz");

        // Create filesystem
        let current_dir = match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => return io_error!("env::current_dir()", e)
        };

        let path = current_dir
            .join("layouts")
            .join(format!("{}.json", self.host));

        let mut fs = filesystem::Filesystem::from_json(&path)?;

        // Open filesystem
        fs.open(&self.password)?;

        thread::sleep(time::Duration::from_secs(1));

        // Create EFI directory
        match fs::create_dir_all(&efi) {
            Ok(_) => log::info!("`{:?}` created", &efi),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Generate initramfs
        self.generate_initramfs(&root, initramfs, &mut fs)?;

        // Close filesystem
        fs.close()?;

        return Success!();
     }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            host: String::from(""),
            password: String::from(""),
            key_file: String::from(""),
            key_filename: String::from(""),
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.host = config.nixos.host;
        self.key_file = config.nixos.key_file;
        self.key_filename = config.nixos.key_filename;

        return Success!();
    }

    fn generate_initramfs(
        &self,
        root: &path::PathBuf,
        initramfs: path::PathBuf,
        fs: &mut filesystem::Filesystem) -> error::Return {

        for disk in fs.disks.iter_mut() {
            if !disk.config.contains_system {
                continue;
            }

            for partition in disk.partitions.iter_mut() {
                if partition.config.is_root {
                    return self.generate_initramfs_in_partition(
                        root,
                        initramfs,
                        partition);
                }

                if !partition.config.is_system {
                    continue;
                }

                for volume in partition.lvm.volumes.iter_mut() {
                    if volume.config.is_root {
                        return self.generate_initramfs_in_volume(
                            root,
                            initramfs,
                            volume);
                    }
                }
            }
        }

        return Success!();
    }

    fn generate_initramfs_in_partition(
        &self,
        root: &path::PathBuf,
        initramfs: path::PathBuf,
        partition: &mut partition::Partition) -> error::Return {

        partition.mount(root)?;

        self.generate_initramfs_to(initramfs)?;

        partition.unmount()?;

        return Success!();
    }

    fn generate_initramfs_in_volume(
        &self,
        root: &path::PathBuf,
        initramfs: path::PathBuf,
        volume: &mut lvm::Volume) -> error::Return {

        volume.mount(root)?;

        self.generate_initramfs_to(initramfs)?;

        volume.unmount()?;

        return Success!();
    }

    fn generate_initramfs_to(&self, output: path::PathBuf) -> error::Return {
        // Cpio
        let mut cpio = match process::Command::new("cpio")
            .arg("-o")
            .arg("-H").arg("newc")
            .arg("-R").arg("+0:+0")
            .arg("--reproducible")
            .arg("--null")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn() {
                Ok(p) => p,
                Err(e) => return cmd_error!("cpio", e),
            };

        let mut cpio_stdin = match cpio.stdin.take() {
            Some(s) => s,
            None => return generic_error!("Cannot obtain access to stdin"),
        };

        match cpio_stdin.write_all(self.key_file.as_bytes()) {
            Ok(_) => (),
            Err(_) => return generic_error!("Cannot write key_file to stdin"),
        }

        drop(cpio_stdin);

        let cpio_output = match cpio.wait_with_output() {
            Ok(o) => o,
            Err(e) => return io_error!("No output for command", e),
        };

        if !cpio_output.status.success() {
            return generic_error!("cpio command returned an error");
        }

        // Gzip
        let mut gzip = match process::Command::new("gzip")
            .arg("-9")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .spawn() {
                Ok(p) => p,
                Err(e) => return cmd_error!("gzip", e),
            };

        let mut gzip_stdin = match gzip.stdin.take() {
            Some(s) => s,
            None => return generic_error!("Cannot obtain access to stdin"),
        };

        match gzip_stdin.write_all(&cpio_output.stdout) {
            Ok(_) => (),
            Err(_) => return generic_error!("Cannot write key_file to stdin"),
        }

        drop(gzip_stdin);

        let gzip_output = match gzip.wait_with_output() {
            Ok(o) => o,
            Err(e) => return io_error!("No output for command", e),
        };

        if !gzip_output.status.success() {
            return generic_error!("gzip command returned an error");
        }

        // Write to file
        let mut file = match fs::File::create(&output) {
            Ok(f) => f,
            Err(e) => return fs_error!(output, e),
        };

        match file.write_all(&gzip_output.stdout) {
            Ok(_) => log::info!("initrd written to {:?}", output),
            Err(e) => return fs_error!(output, e),
        }

        return Success!();
    }
}
