// -----------------------------------------------------------------------------

use clap;
use std::fs;
use std::path;

use super::env;
use super::error;
use super::filesystem;
use super::gpt;
use super::partition;
use std::str::FromStr;
use super::traits::{CliCommand, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_HOST: &str = "host";

// -----------------------------------------------------------------------------

/// Command structure for creating filesystems configurations for NixOS
#[derive(Debug)]
pub struct Command {
    /// Host name
    host: String,

    /// Name of the key file used to decrypt disks
    key_filename: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return
            !self.host.is_empty() &&
            !self.key_filename.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "filesystems";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Create filesystems configurations for NixOS")
            .version(version)
            .author(author)
            // Host argument
            .arg(clap::Arg::with_name(ARG_HOST)
                .long(ARG_HOST)
                .help("Host name (optional if a .env file is present)")
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

        // Create filesystem from Json description
        let path = utils::current_dir()?
            .join("layouts")
            .join(format!("{}.json", self.host));

        let fs = filesystem::Filesystem::from_json(&path)?;

        // Create output directories
        let output = utils::current_dir()?
            .join("filesystems")
            .join(format!("{}", self.host));

        match fs::create_dir_all(&output) {
            Ok(_) => (),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Create configurations
        self.create_default(&output)?;
        self.create_bootloader(&output)?;
        self.create_devices(&fs, &output)?;
        self.create_filesystems(&fs, &output)?;

        return Success!();
    }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            host: String::from(""),
            key_filename: String::from(""),
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.host = config.nixos.host;
        self.key_filename = config.nixos.key_filename;

        return Success!();
    }

    /// Create the `default.nix` file in provided directory
    fn create_default(&self, path: &path::PathBuf) -> error::Return {
        let content =
r"# Auto-generated, do not edit !
{ ... }:

{
  imports = [
    ./bootloader.nix
    ./devices.nix
    ./filesystems.nix
  ];
}";

        let output = path.join("default.nix");

        utils::write_to_file(content.as_bytes(), &output)?;

        log::info!("{}", content);
        log::info!("Configuration written to {}", output.to_str().unwrap());

        return Success!();
    }

    /// Create the `bootloader.nix` file in provided directory
    fn create_bootloader(&self, path: &path::PathBuf) -> error::Return {
        //TODO: remove zfsSupport ?
        let content =
r#"# Auto-generated, do not edit !
{ config, ... }:

{
  boot.loader = {
    timeout = 1;

    efi = {
      canTouchEfiVariables = true;
      efiSysMountPoint = "/boot/efi";
    };

    grub = {
      enable = true;
      device = "nodev";
      version = 2;
      efiSupport = true;
      enableCryptodisk = true;
      copyKernels = true;
      zfsSupport = true;
    };
  };
}"#;

        let output = path.join("bootloader.nix");

        utils::write_to_file(content.as_bytes(), &output)?;

        log::info!("{}", content);
        log::info!("Configuration written to {}", output.to_str().unwrap());

        return Success!();
    }

    /// Create `devices.nix` file in provided directory
    fn create_devices(
        &self,
        fs: &filesystem::Filesystem,
        path: &path::PathBuf) -> error::Return {

        let mut content = "# Auto-generated, do not edit !\n".to_string();
        content += "{ config, ... }:\n\n";
        content += "{\n";
        content += "  boot = {";

        if self.has_zfs(fs) {
            content += "\n";
            content += r#"    supportedFilesystems = ["zfs"];"#;
            content += "\n";
        }

        content += "\n";
        content += "    initrd = {";

        if self.is_root_zfs(fs) {
            content += "\n";
            content += r#"      supportedFilesystems = ["zfs"];"#;
            content += "\n";
        }

        for disk in fs.disks.iter() {
            for partition in disk.partitions.iter() {
                if !partition.config.encrypted {
                    continue;
                }

                let device = match &partition.config.device_by_partlabel {
                    Some(d) => d,
                    None => return generic_error!("No path for partition"),
                };

                content += "\n";
                content += &format!(
                    r#"      luks.devices."{}" = {{"#,
                    partition.config.label);

                content += "\n";
                content += &format!(r#"        device = "{}";"#, device);

                content += "\n";
                content += &format!(
                    r#"        keyFile = "/{}";"#,
                    self.key_filename);

                content += "\n";
                content += "        allowDiscards = true;";

                content += "\n";
                content += "        preLVM = true;";

                content += "\n";
                content += "      };\n";
            }
        }

        content += "\n";
        content += "      secrets = {";

        content += "\n";
        content += &format!(
            r#"        "/{0}" = "/etc/secrets/disks/{0}";"#,
            &self.key_filename);

        content += "\n";
        content += "      };";

        content += "\n";
        content += "    };";

        content += "\n";
        content += "  };";

        content += "\n";
        content += "}";

        log::info!("{}", content);

        // Write to file
        let output = path.join("devices.nix");

        utils::write_to_file(content.as_bytes(), &output)?;

        log::info!("Configuration written to {:?}", &output);

        return Success!();
    }

    /// Create `filesystems.nix` file in provided directory
    fn create_filesystems(
        &self,
        fs: &filesystem::Filesystem,
        path: &path::PathBuf) -> error::Return {

        let host_id = self.get_host_id()?;

        let mut content = "# Auto-generated, do not edit !\n".to_string();
        content += "{ config, ... }:\n\n";
        content += "{\n";
        content += &format!(r#"  networking.hostId = "{}";"#, host_id);

        for disk in fs.disks.iter() {
            for partition in disk.partitions.iter() {
                match partition.config.partition_type.as_str() {
                    "linux" => {
                        content += &self.create_fs_from_partition(&partition)?;
                    },

                    "efi" => {
                        content +=
                            &self.create_fs_from_efi_partition(&partition)?;
                    }

                    _ => {},
                }
            }
        }

        content += "\n}";

        log::info!("{}", content);

        // Write to file
        let output = path.join("filesystems.nix");

        utils::write_to_file(content.as_bytes(), &output)?;

        log::info!("Configuration written to {:?}", &output);

        return Success!();
    }

    /// Create filesystem entry from partition
    fn create_fs_from_partition(
        &self,
        partition: &partition::Partition) -> Result<String, error::Error> {

        return match gpt::FsType::from_str(&partition.config.fs_type)? {
            gpt::FsType::Zfs => self.create_fs_from_zfs_partition(partition),
            _ => self.create_fs_from_basic_partition(partition),
        }
    }

    /// Create filesystem entry from EFI partition
    fn create_fs_from_efi_partition(
        &self,
        partition: &partition::Partition) -> Result<String, error::Error> {

        let mut content = "\n\n".to_string();
        content += r#"  fileSystems."/boot/efi" = {"#;
        content += "\n";
        content += &format!(
            r#"    device = "{}";"#,
            partition.config.device_by_partlabel.as_ref().unwrap());
        content += "\n";
        content += r#"    fsType = "vfat";"#;
        content += "\n";
        content += "  };";

        return Ok(content);
    }

    /// Create filesystem entry from non-ZFS partition
    fn create_fs_from_basic_partition(
        &self,
        p: &partition::Partition) -> Result<String, error::Error> {

        let device = match p.config.encrypted {
            true => p.config.luks_mapper.as_ref().unwrap(),
            false => p.config.device_by_partlabel.as_ref().unwrap(),
        };

        let mut content = "\n\n".to_string();
        content += &format!(r#"  fileSystems."{}" = {{"#, &p.config.label);

        content += "\n";
        content += &format!(r#"    device = "{}";"#, &device);

        if p.config.encrypted {
            let blk_dev = p.config.device_by_partlabel.as_ref().unwrap();

            content += "\n\n";
            content += "    encrypted = {";

            content += "\n";
            content += "      enable = true;";

            content += "\n";
            content += &format!(r#"      blkdev = "{}";"#, &blk_dev);

            content += "\n";
            content += &format!(
                r#"      label = "{}";"#,
                &p.config.label);

            content += "\n";
            content += &format!(
                r#"      keyFile = "/etc/secrets/disks/{}";"#,
                &self.key_filename);

            content += "\n";
            content += "    };";
        }

        content += "\n";
        content += "  };";

        return Ok(content);
    }

    /// Create filesystem entry from ZFS partition
    fn create_fs_from_zfs_partition(
        &self,
        p: &partition::Partition) -> Result<String, error::Error> {

        let mut content = "".to_string();

        for fs in p.config.zfs.iter() {
            let device = format!("{}/{}", p.config.label, fs.name);

            content += "\n\n";
            content += &format!(r#"  fileSystems."{}" = {{"#, &fs.mountpoint);

            content += "\n";
            content += &format!(r#"    device = "{}";"#, &device);

            content += "\n";
            content += r#"    fsType = "zfs";"#;

            content += "\n";
            content += "  };";
        }

        return Ok(content);
    }

    /// Create a unique host identifier
    fn get_host_id(&self) -> Result<String, error::Error> {
        let output = utils::command_output(
            "head",
            &[
                "-c", "8",
                "/etc/machine-id"
            ])?;

        let id = utils::command_stdout_to_string(&output)?;

        return Ok(id);
    }

    /// Check if the filesystem contains at least one ZFS
    fn has_zfs(&self, fs: &filesystem::Filesystem) -> bool {
        for disk in fs.disks.iter() {
            for p in disk.partitions.iter() {
                let fs_type = match gpt::FsType::from_str(&p.config.fs_type) {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                match fs_type {
                    gpt::FsType::Zfs => return true,
                    _ => continue,
                }
            }
        }

        return false;
    }

    /// Check if the root partition/filesystem is a ZFS
    fn is_root_zfs(&self, fs: &filesystem::Filesystem) -> bool {
        for disk in fs.disks.iter() {
            for p in disk.partitions.iter() {
                let fs_type = match gpt::FsType::from_str(&p.config.fs_type) {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                if fs_type != gpt::FsType::Zfs {
                    continue;
                }

                for fs in p.config.zfs.iter() {
                    if fs.is_root {
                        return true;
                    }
                }
            }
        }

        return false;
    }
}
