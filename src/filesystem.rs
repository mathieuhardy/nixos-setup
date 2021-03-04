// -----------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path;

use super::disk;
use super::error;
use super::traits::{Configurable, Openable, Validate};
use super::utils;
use super::zfs;

// -----------------------------------------------------------------------------

/// Json configuration of the filesystem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// List of disks configurations
    disks: Vec<disk::Config>,
}

impl Validate for Config {
    fn is_valid(&self) -> bool {
        for d in self.disks.iter() {
            if !d.is_valid() {
                return false;
            }
        }

        return true;
    }
}

// -----------------------------------------------------------------------------

/// Filesystem representation
#[derive(Debug)]
pub struct Filesystem {
    /// List of disks i the filesystem
    pub disks: Vec<disk::Disk>,
}

impl Filesystem {
    /// Create all the filesystem
    pub fn create(
        &mut self,
        key_file: &str,
        passphrase: &str) -> error::Return {

        zfs::wipeout()?;

        for disk in self.disks.iter_mut() {
            if !disk.read_only() {
                disk.wipeout()?;
                disk.create(key_file, passphrase)?;
            }
        }

        log::info!("{:#?}", self.to_config());

        return Success!();
    }

    /// Load Json file and create filesystem objects
    pub fn from_json(json: &path::PathBuf) -> Result<Self, error::Error> {

        let config: Config = match utils::load_json(json) {
            Ok(j) => j,
            Err(e) => return Err(e),
        };

        log::info!("{:#?}", config);

        if !config.is_valid() {
            return generic_error!("Filesystem configuration is not valid");
        }

        return Ok(Self::from_config(config));
    }

    /// Export filesystem to Json file
    pub fn to_json(&self, json: &path::PathBuf) -> error::Return {
        let value = utils::json_to_string(&self.to_config()?)?;

        utils::write_to_file(value.as_bytes(), json)?;

        log::info!("Configuration has been written to {:?}", json);

        return Success!();
    }

    /// Provide the device mapping
    pub fn set_device_mapping(&mut self, mapping: &HashMap<String, String>) {
        for disk in self.disks.iter_mut() {
            let device = &disk.config.device;

            if !device.starts_with("#") {
                continue;
            }

            let key = device.trim_start_matches("#");

            if !mapping.contains_key(key) {
                continue;
            }

            disk.config.device = mapping[key].clone();
        }
    }

    /// Create configuration from filesystem
    pub fn to_config(&self) -> Result<Config, error::Error> {
        let mut disks = Vec::new();

        for disk in &self.disks {
            disks.push(disk.config()?);
        }

        let config = Config {
            disks: disks,
        };

        return Ok(config);
    }


    /// Find the system disk
    pub fn find_system_disk(&mut self)
        -> Result<&mut disk::Disk, error::Error> {

        for disk in self.disks.iter_mut() {
            if disk.config.contains_system {
                return Ok(disk);
            }
        }

        return generic_error!("System disk not found");
    }

    /// Create filesystem from configuration
    fn from_config(config: Config) -> Self {
        let mut disks = Vec::new();

        for d in config.disks.iter() {
            disks.push(disk::Disk::from_config(d));
        }

        Self {
            disks: disks,
        }
    }
}

impl Openable for Filesystem {
    fn open(&mut self, passphrase: &str) -> error::Return {
        // Open each disk
        for disk in self.disks.iter_mut() {
            disk.open(passphrase)?;
        }

        // Open all ZFS
        zfs::pool_import_all()?;

        return Success!();
    }

    fn close(&mut self) -> error::Return {
        // Close all ZFS
        zfs::pool_export_all()?;

        // Close each disk
        for disk in self.disks.iter_mut() {
            disk.close()?;
        }

        return Success!();
    }
}
