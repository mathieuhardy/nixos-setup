// -----------------------------------------------------------------------------

use clap;
use std::fs;
use std::path;
use std::thread;
use std::time;

use super::env;
use super::filesystem;
use super::error;
use super::lvm;
use super::partition;
use super::traits::{CliCommand, Mountable, Openable, Validate};
use super::utils;
use super::zfs;

// -----------------------------------------------------------------------------

const ARG_HOST: &str = "host";
const ARG_PASSWORD: &str = "password";

// -----------------------------------------------------------------------------

/// Command structure installing secrets on the filesystem
#[derive(Debug)]
pub struct Command {
    /// Host name
    host: String,

    /// Password used to decrypt disks
    password: String,

    /// Key file to install
    key_file: String,

    /// File name of the key
    key_filename: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return
            !self.host.is_empty() &&
            !self.key_file.is_empty() &&
            !self.key_filename.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "secrets";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Install secrets")
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

    /// Process command line arguments
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

        // Check validity
        if !self.is_valid() {
            return generic_error!("Invalid configuration");
        }

        // Create root directory
        let root = path::Path::new("/").join("mnt").join("root");

        match fs::create_dir_all(&root) {
            Ok(_) => log::info!("`{:?}` created", &root),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Create filesystem
        let json = utils::current_dir()?
            .join("layouts")
            .join(format!("{}.json", self.host));

        let mut fs = filesystem::Filesystem::from_json(&json)?;

        // Open filesystem
        fs.open(&self.password)?;

        thread::sleep(time::Duration::from_secs(1));

        // Install key file
        self.install_keyfile(&root, &mut fs)?;

        // Close filesystem
        fs.close()?;

        return Success!();
    }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            host: "".to_string(),
            password: "".to_string(),
            key_file: "".to_string(),
            key_filename: "".to_string(),
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

    /// Install the key file on the filesystem
    fn install_keyfile(
        &self,
        root: &path::PathBuf,
        fs: &mut filesystem::Filesystem) -> error::Return {

        for disk in fs.disks.iter_mut() {
            if !disk.config.contains_system {
                continue;
            }

            for partition in disk.partitions.iter_mut() {
                if partition.config.is_root {
                    return self.install_keyfile_in_partition(root, partition);
                }

                if !partition.config.is_system {
                    continue;
                }

                for volume in partition.lvm.volumes.iter_mut() {
                    if volume.config.is_root {
                        return self.install_keyfile_in_volume(root, volume);
                    }
                }

                for filesystem in partition.zfs.filesystems.iter_mut() {
                    if filesystem.config.is_root {
                        return self.install_keyfile_in_zfs_fs(root, filesystem);
                    }
                }
            }
        }

        return Success!();
    }

    /// Install the key file in the partition
    fn install_keyfile_in_partition(
        &self,
        root: &path::PathBuf,
        partition: &mut partition::Partition) -> error::Return {

        partition.mount(root)?;

        self.install_keyfile_to(root)?;

        partition.unmount()?;

        return Success!();
    }

    /// Install the key file in the logical volume
    fn install_keyfile_in_volume(
        &self,
        root: &path::PathBuf,
        volume: &mut lvm::Volume) -> error::Return {

        volume.mount(root)?;

        self.install_keyfile_to(root)?;

        volume.unmount()?;

        return Success!();
    }

    /// Install the key file in the ZFS filesystem
    fn install_keyfile_in_zfs_fs(
        &self,
        root: &path::PathBuf,
        fs: &mut zfs::Filesystem) -> error::Return {

        fs.mount(root)?;

        self.install_keyfile_to(root)?;

        fs.unmount()?;

        return Success!();
    }

    /// Install the key file to the given path
    fn install_keyfile_to(&self, root: &path::PathBuf) -> error::Return {
        // Create diretory
        let install_path = root.join("etc").join("secrets").join("disks");

        match fs::create_dir_all(&install_path) {
            Ok(_) => (),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Install key file
        let dest = install_path.join(&self.key_filename);

        match fs::copy(&self.key_file, &dest) {
            Ok(_) => (),
            Err(e) => return io_error!("Error installing keyfile", e),
        }

        // Set permissions
        let path = match install_path.join(&dest).to_str() {
            Some(m) => m.to_string(),
            None => return generic_error!("No path"),
        };

        log::info!("Successfully installed key to {}", path);

        utils::command_output("chmod", &["000", &path])?;

        log::info!("Successfully changed permissions");

        return Success!();
    }
}
