// -----------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::path;

use super::error;
use super::traits::{Mountable, Validate};
use super::utils;

// -----------------------------------------------------------------------------

/// Json configuration of a ZFS filesystem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config{
    /// Name of the filesystem
    pub name: String,

    /// Mountpoint of the filesystem
    pub mountpoint: String,

    /// Whether this partition is the root mount point
    pub is_root: bool,
}

impl Validate for Config{
    fn is_valid(&self) -> bool {
        return
            !self.name.is_empty() &&
            !self.mountpoint.is_empty();
    }
}

// -----------------------------------------------------------------------------

/// Filesystems representation
#[derive(Debug)]
pub struct Filesystems {
    /// List of file systems
    pub filesystems: Vec<Filesystem>,
}

impl Filesystems {
    /// Create filesystems entries from Json configuration
    pub fn from_config(pool : &str, configs : &Vec<Config>) -> Self {
        let mut filesystems : Vec<Filesystem> = Vec::new();

        for config in configs.iter() {
            filesystems.push(Filesystem::from_config(pool,config));
        }

        Self {
            filesystems: filesystems,
        }
    }

    /// Convert to Json configuration
    pub fn config(&self) -> Result<Vec<Config>, error::Error> {
        let mut config : Vec<Config> = Vec::new();

        for fs in self.filesystems.iter() {
            config.push(fs.config()?);
        }

        return Ok(config);
    }

    /// Create filesystems
    pub fn create(&mut self) -> error::Return {
        for fs in self.filesystems.iter_mut() {
            fs.create()?;
        }

        return Success!();
    }
}

impl Validate for Filesystems{
    fn is_valid(&self) -> bool {
        return !self.filesystems.is_empty();
    }
}

// -----------------------------------------------------------------------------

/// Filesystem representation
#[derive(Debug)]
pub struct Filesystem {
    /// Json configuration
    pub config: Config,

    /// Pool name
    pub pool: String,

    /// Whether the filesystem is opened or not
    opened: bool,

    /// Wether the filesystem is mounted or not
    mounted: bool,
}

impl Filesystem {
    /// Create filesystem entry from Json configuration
    pub fn from_config(pool: &str, config: &Config) -> Self {
        Self {
            config: config.clone(),
            pool: pool.to_string(),
            opened: false,
            mounted: false,
        }
    }

    /// Convert to Json configuration
    pub fn config(&self) -> Result<Config, error::Error> {
        return Ok(Config {
            name: self.config.name.clone(),
            mountpoint: self.config.mountpoint.clone(),
            is_root: self.config.is_root.clone(),
        });
    }

    /// Create filesystem
    pub fn create(&mut self) -> error::Return {
        zfs_create(&self.pool, &self.config.name)?;

        return Success!();
    }
}

impl Mountable for Filesystem {
    /// Mount this partition
    fn mount(&mut self, mountpoint: &path::PathBuf) -> error::Return {

        if self.mounted {
            return Success!();
        }

        let device = format!("{}/{}", self.pool, self.config.name);

        let mountpoint = match mountpoint.to_str() {
            Some(m) => m,
            None => return generic_error!("No mountpoint"),
        };

        utils::command_output("mount", &["-t", "zfs", &device, mountpoint])?;

        self.mounted = true;

        log::info!("`{}` mounted to `{}`", device, mountpoint);

        return Success!();
    }

    /// Unmount this partition
    fn unmount(&mut self) -> error::Return {
        if !self.mounted {
            return Success!();
        }

        let device = format!("{}/{}", self.pool, self.config.name);

        utils::command_output("umount", &[&device])?;

        self.mounted = false;

        log::info!("{} unmounted", device);

        return Success!();
    }
}

// -----------------------------------------------------------------------------

pub fn pool_create(name : &str, device : &str) -> error::Return {
    pool_import_all()?;

    if pool_exists(name) {
        return pool_add(name, device);
    }

    pool_export_all()?;

    utils::command_output(
        "zpool",
        &[
            "create",
            "-o", "ashift=12",
            "-O", "compression=lz4",
            "-m", "none",
            name,
            device,
        ])?;

    return Success!();
}

pub fn pool_add(name : &str, device : &str) -> error::Return {
    utils::command_output("zpool", &["add", "-f", name, device])?;

    return Success!();
}

pub fn pool_destroy(name : &str) -> error::Return {
    utils::command_output("zpool", &["destroy", "-f", name])?;

    return Success!();
}

pub fn pool_import_all() -> error::Return {
    utils::command_output("zpool", &["import", "-a"])?;

    return Success!();
}

//pub fn pool_export(pool : &str) -> error::Return {
    //utils::command_output("zpool", &["export", "-f", pool])?;

    //return Success!();
//}

pub fn pool_export_all() -> error::Return {
    utils::command_output("zpool", &["export", "-a"])?;

    return Success!();
}

pub fn zfs_create(pool : &str, name : &str) -> error::Return {
    let path = format!("{}/{}", pool, name);

    utils::command_output(
        "zfs",
        &[
            "create",
            &path,
            "-o",
            "mountpoint=legacy"
        ])?;

    log::info!("ZFS filesystem `{}` created", path);

    return Success!();
}

pub fn wipeout() -> error::Return {
    let output = utils::command_output("zpool", &["list", "-H", "-o", "name"])?;
    let output = utils::command_stdout_to_string(&output)?;

    for pool in output.lines() {
        pool_destroy(pool)?;

        log::info!("{} destroyed", pool);
    }

    return Success!();
}

pub fn pool_exists(name : &str) -> bool {
    return match utils::command_output("zpool", &["list", name]) {
        Ok(_) => true,
        Err(_) => false,
    };
}
