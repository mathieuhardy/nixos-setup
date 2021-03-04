// -----------------------------------------------------------------------------

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path;
use std::str::FromStr;

use super::error;
use super::gpt;
use super::luks;
use super::lvm;
use super::traits::{Configurable, Mountable, Openable, Validate};
use super::utils;
use super::zfs;

// -----------------------------------------------------------------------------

/// Json configuration of a partition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config{
    /// Unique identifier of th partition (starts at 1)
    pub id: u32,

    /// Size of the partition
    pub size: gpt::Bytesize,

    /// Type of the partition
    pub partition_type: String,

    /// Whether the partition is encrypted or not
    pub encrypted: bool,

    /// Type of filesystem of the partition
    pub fs_type: String,

    /// Label of the partition
    pub label: String,

    /// Whether this partition hosts the Linux system
    pub is_system: bool,

    /// Whether this partition is the root mount point
    pub is_root: bool,

    /// LVM configuration
    pub lvm: Vec<lvm::Config>,

    /// ZFS filesystems
    pub zfs: Vec<zfs::Config>,

    /// Block device of this partition
    pub device: Option<String>,

    /// Name of the block device
    pub device_name: Option<String>,

    /// Block device of this partition (by id)
    pub device_by_id: Option<String>,
    
    /// Block device of this partition (by partlabel)
    pub device_by_partlabel: Option<String>,

    /// Mapper device for LUKS partition
    pub luks_mapper: Option<String>,
}

impl Validate for Config{
    fn is_valid(&self) -> bool {
        if self.id == 0 {
            return false;
        }

        match gpt::PartitionType::from_str(&self.partition_type) {
            Ok(_) => (),
            Err(_) => return false,
        };

        match gpt::FsType::from_str(&self.fs_type) {
            Ok(_) => (),
            _ => return false,
        }

        if self.label.is_empty() {
            return false;
        }

        return true;
    }
}

// -----------------------------------------------------------------------------

/// Partition representation
#[derive(Debug)]
pub struct Partition {
    /// Json configuration
    pub config: Config,

    /// Whether the partition is opened or not
    opened: bool,

    /// Wether the partition is mounted or not
    mounted: bool,

    /// Optional LVM entry
    pub lvm: lvm::Lvm,

    /// ZFS filesystems
    pub zfs: zfs::Filesystems,
}

impl Partition {
    /// Create partition
    pub fn create(&mut self, device: &str) -> error::Return {
        // Create
        gpt::create_partition(
            device,
            &self.config.size,
            &gpt::PartitionType::from_str(&self.config.partition_type)?,
            &self.config.label)?;

        // Identify partition device
        self.identify(device)?;

        // Identify partition id
        self.identify_id()?;

        // Set LUKS mapper (if needed)
        if self.config.encrypted {
            self.config.luks_mapper =
                Some(format!("/dev/mapper/{}", self.config.label));
        }

        return Success!();
    }

    /// Format partition
    pub fn format(
        &mut self,
        key_file: &str,
        passphrase: &str) -> error::Return {

        // LUKS initialize
        self.luks_format(passphrase, key_file)?;

        // Get device regarding encryption
        let device = match self.config.encrypted {
            false => self.config.device_by_id.as_ref().unwrap().clone(),
            true => self.config.luks_mapper.as_ref().unwrap().clone(),
        };

        // Format filesystem
        match self.lvm.is_valid() {
            true => {
                self.lvm.create(&device, &self.config.label)?;
                self.lvm.format_volumes()?;
            },

            false => {
                gpt::format_partition(
                    &device,
                    &self.config.fs_type,
                    &self.config.label)?;
            },
        }

        // ZFS filesystems
        if self.zfs.is_valid() {
            self.zfs.create()?;
        }

        return Success!();
    }

    /// Identify the block device of this partition
    fn identify(&mut self, device: &str) -> error::Return {
        // Run command
        let output = utils::command_output("fdisk", &["-l", device])?;

        let stdout = utils::command_stdout_to_string(&output)?;

        // Search partition
        let pattern = format!(r"({}[^ ]*{})", device, self.config.id);

        let re = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(e) => return generic_error!(
                &format!("Cannot build regex: {}", e.to_string())),
        };

        let captures = match re.captures(&stdout) {
            Some(c) => c,
            None => return generic_error!("Cannot identify partition"),
        };

        let partition_device = captures.get(0).map_or("", |m| m.as_str());

        if partition_device.is_empty() {
            return generic_error!("No partition found");
        }

        self.config.device = Some(partition_device.to_string());

        self.config.device_name =
            Some(partition_device.to_string().replace("/dev/", ""));

        log::info!(
            "Partition `{}` identified on device `{}`",
            self.config.label,
            partition_device);

        return Success!();
    }

    /// Identify ID of this partition
    fn identify_id(&mut self) -> error::Return {
        // Run command
        let output = utils::command_output("ls", &["-l", "/dev/disk/by-id"])?;
        let output = utils::command_stdout_to_string(&output)?;

        // Search device
        let device = self.config.device_name.as_ref().unwrap();

        let pattern = format!(r"([^ ]*) -> .*{}$", device);

        let re = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(e) => return generic_error!(
                &format!("Cannot build regex: {}", e.to_string())),
        };

        for line in output.lines() {
            let captures = match re.captures(&line) {
                Some(c) => c,
                None => continue,
            };

            let id = captures.get(1).map_or("", |m| m.as_str());

            if id.is_empty() {
                return generic_error!("No partition id");
            }

            self.config.device_by_id = Some(format!("/dev/disk/by-id/{}", &id));

            self.config.device_by_partlabel =
                Some(format!("/dev/disk/by-partlabel/{}", &self.config.label));

            log::info!(
                "Partition `{}` identified on device `{}`",
                self.config.label,
                self.config.device_by_id.as_ref().unwrap());

            return Success!();
        }

        return generic_error!("Cannot find partition ID");
    }

    /// Format this partition using LUKS
    fn luks_format(&mut self, passphrase: &str, key_file: &str) -> error::Return {
        if self.config.encrypted == false {
            return Success!();
        }

        // Get device to setup
        let device = self.config.device_by_id.as_ref().unwrap();

        // Format
        luks::format(device, passphrase)?;

        // Add key file
        luks::add_key(device, passphrase, key_file)?;

        // Open
        luks::open(
            self.config.device_by_id.as_ref().unwrap(),
            passphrase,
            &self.config.label)?;

        self.opened = true;

        return Success!();
    }
}

impl Mountable for Partition {
    /// Mount this partition
    fn mount(&mut self, mountpoint: &path::PathBuf) -> error::Return {
        if self.mounted {
            return Success!();
        }

        let device = self.config.device_by_id.as_ref().unwrap();

        let mountpoint = match mountpoint.to_str() {
            Some(m) => m,
            None => return generic_error!("No mountpoint"),
        };

        utils::command_output("mount", &[device, mountpoint])?;

        self.mounted = true;

        log::info!("`{}` mounted to `{}`", device, mountpoint);

        return Success!();
    }

    /// Unmount this partition
    fn unmount(&mut self) -> error::Return {
        if !self.mounted {
            return Success!();
        }

        let device = match &self.config.device_by_id {
            Some(d) => d,
            None => return generic_error!("No device for partition"),
        };

        utils::command_output("umount", &[device])?;

        self.mounted = false;

        log::info!("{} unmounted", device);

        return Success!();
    }
}

impl Openable for Partition {
    fn open(&mut self, passphrase: &str) -> error::Return {
        if self.opened {
            return Success!();
        }

        // Open LUKS (if needed)
        if self.config.encrypted {
            luks::open(
                self.config.device_by_id.as_ref().unwrap(),
                passphrase,
                &self.config.label)?;
        }

        // Open LVM (if needed)
        if self.lvm.is_valid() {
            self.lvm.open(passphrase)?;
        }

        self.opened = true;

        log::info!("Partition `{}` opened", self.config.label);

        return Success!();
    }

    fn close(&mut self) -> error::Return {
        if !self.opened {
            return Success!();
        }

        // Close LVM (if needed)
        if self.lvm.is_valid() {
            self.lvm.close()?;
        }

        // Close LUKS (if needed)
        if self.config.encrypted {
            luks::close(&self.config.label)?;
        }

        self.opened = false;

        log::info!("Partition `{}` closed", self.config.label);

        return Success!();
    }
}

impl Configurable<Config> for Partition {
    fn from_config(config: &Config) -> Self {
        Self {
            config: config.clone(),
            opened: false,
            mounted: false,
            lvm: lvm::Lvm::from_config(&config.lvm, &config.label),
            zfs: zfs::Filesystems::from_config(&config.label, &config.zfs),
        }
    }

    fn config(&self) -> Result<Config, error::Error> {
        return Ok(Config {
            id: self.config.id.clone(),
            size: self.config.size.clone(),
            partition_type: self.config.partition_type.clone(),
            encrypted: self.config.encrypted.clone(),
            fs_type: self.config.fs_type.clone(),
            label: self.config.label.clone(),
            is_system: self.config.is_system.clone(),
            is_root: self.config.is_root.clone(),
            lvm: self.lvm.config()?,
            zfs: self.zfs.config()?,
            device: self.config.device.clone(),
            device_name: self.config.device_name.clone(),
            device_by_id: self.config.device_by_id.clone(),
            device_by_partlabel: self.config.device_by_partlabel.clone(),
            luks_mapper: self.config.luks_mapper.clone(),
        });
    }
}
