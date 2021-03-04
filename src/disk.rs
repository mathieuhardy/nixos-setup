// -----------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::error;
use super::gpt;
use super::partition;
use super::traits::{Configurable, Mountable, Openable, Validate};

// -----------------------------------------------------------------------------

/// Json configuration of a disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Path of the disk device
    pub device: String,

    /// If ready-only: no write operation will be performed on this disk
    pub read_only: bool,

    /// Whether this disk contains the Linux system
    pub contains_system: bool,

    /// List of partition configurations
    pub partitions: Vec<partition::Config>,
}

impl Validate for Config {
    fn is_valid(&self) -> bool {
        if self.device.is_empty() {
            return false;
        }

        for p in self.partitions.iter() {
            if !p.is_valid() {
                return false;
            }
        }

        return true;
    }
}

// -----------------------------------------------------------------------------

/// Disk representation
#[derive(Debug)]
pub struct Disk {
    /// Disk configuration
    pub config: Config,

    /// List of partitions
    pub partitions: Vec<partition::Partition>,
}

impl Disk {
    /// Check if disk is read-only
    pub fn read_only(&self) -> bool {
        self.config.read_only
    }

    /// Wipeout the disk
    pub fn wipeout(&self) -> error::Return {
        return gpt::wipeout(&self.config.device);
    }

    /// Create the disk from its configuration
    pub fn create(
        &mut self,
        key_file: &str,
        passphrase: &str) -> error::Return {

        // Create
        for partition in self.partitions.iter_mut() {
            partition.create(&self.config.device)?;
        }

        // Format
        for partition in self.partitions.iter_mut() {
            partition.format(key_file, passphrase)?;
        }

        return Success!();
    }

    /// Find root partition/lvm/zfs
    pub fn find_root_partition(&mut self)
        -> Result<&mut dyn Mountable, error::Error> {

        for p in self.partitions.iter_mut() {
            if p.config.is_root {
                return Ok(p);
            }

            if !p.config.is_system {
                continue;
            }

            for volume in p.lvm.volumes.iter_mut() {
                if volume.config.is_root {
                    return Ok(volume);
                }
            }

            for fs in p.zfs.filesystems.iter_mut() {
                if fs.config.is_root {
                    return Ok(fs);
                }
            }
        }

        return generic_error!("Root partition not found");
    }

    /// Find EFI partition/lvm/zfs
    pub fn find_efi_partition(&mut self)
        -> Result<&mut dyn Mountable, error::Error> {

        for p in self.partitions.iter_mut() {
            let partition_type =
                gpt::PartitionType::from_str(&p.config.partition_type)?;

            match partition_type {
                gpt::PartitionType::Efi => return Ok(p),
                _ => (),
            }

            if !p.config.is_system {
                continue;
            }

            for volume in p.lvm.volumes.iter_mut() {
                let volume_type =
                    gpt::PartitionType::from_str(&volume.config.volume_type)?;

                match volume_type {
                    gpt::PartitionType::Efi => return Ok(volume),
                    _ => (),
                }
            }

            // Cannot be ZFS so no need to check
        }

        return generic_error!("EFI partition not found");
    }
}

impl Openable for Disk {
    fn open(&mut self, passphrase: &str) -> error::Return {
        for partition in self.partitions.iter_mut() {
            partition.open(passphrase)?;
        }

        log::info!("Disk `{}` opened", self.config.device);

        return Success!();
    }

    fn close(&mut self) -> error::Return {
        for partition in self.partitions.iter_mut() {
            partition.close()?;
        }

        log::info!("Disk `{}` closed", self.config.device);

        return Success!();
    }
}

impl Configurable<Config> for Disk {
    fn from_config(config: &Config) -> Self {
        // First sort partitions by id
        let mut c = config.clone();

        c.partitions.sort_by_key(|k| k.id);

        // Create list of partitions
        let mut partitions = Vec::new();

        for p in c.partitions.iter() {
            partitions.push(partition::Partition::from_config(p));
        }

        // Return instance
        Self {
            config: c,
            partitions: partitions,
        }
    }

    fn config(&self) -> Result<Config, error::Error> {
        let mut partitions = Vec::new();

        for partition in &self.partitions {
            match partition.config() {
                Ok(c) => partitions.push(c),
                Err(_) => return generic_error!("No partition configuration"),
            }
        }

        return Ok(Config {
            device: self.config.device.clone(),
            read_only: self.config.read_only.clone(),
            contains_system: self.config.contains_system.clone(),
            partitions: partitions,
        });
    }
}
