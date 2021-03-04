// -----------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::path;

use super::error;
use super::gpt;
use super::traits::{Configurable, Mountable, Openable, Validate};
use super::utils;

// -----------------------------------------------------------------------------

/// Json configuration of a LVM volume
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Identifier of the volume
    pub id: u32,

    /// Size of the volume
    pub size: gpt::Bytesize,

    /// Type of the logical volume
    pub volume_type: String,

    /// Whether the volume is encrypted or not
    pub encrypted: bool,

    /// Filesystem type of the volume
    pub fs_type: String,

    /// Label of the volume
    pub label: String,

    /// Wether the volume is the root filesystem
    pub is_root: bool,

    /// Block device of the volume
    pub device: Option<String>,
}

// -----------------------------------------------------------------------------

/// LVM entry
#[derive(Debug)]
pub struct Lvm {
    /// Whether the LVM is opened or not
    opened: bool,

    /// Label of the partition
    partition_label: String,

    /// List of logical volumes
    pub volumes: Vec<Volume>,
}

impl Lvm {
    /// Create a LVM from Json configuration
    pub fn from_config(lvms : &Vec<Config>, partition_label: &str) -> Self {
        let mut volumes = Vec::new();

        for lvm in lvms.iter() {
            volumes.push(Volume::from_config(lvm));
        }

        Self {
            volumes: volumes,
            partition_label: partition_label.to_string(),
            opened: false,
        }
    }

    /// Convert LVM to Json configuration
    pub fn config(&self) -> Result<Vec<Config>, error::Error> {
        let mut lvms = Vec::new();

        for volume in self.volumes.iter() {
            lvms.push(volume.config()?);
        }

        return Ok(lvms);
    }

    /// Create the LVM
    pub fn create(&mut self, device: &str, label: &str) -> error::Return {
        if !self.is_valid() {
            return Success!();
        }

        self.pv_create(device)?;
        self.vg_create(device, label)?;
        self.volumes_create(label)?;

        self.opened = true;

        return Success!();
    }

    /// Format volumes of the LVM
    pub fn format_volumes(&self) -> error::Return {
        for volume in self.volumes.iter() {
            volume.format()?;
        }

        return Success!();
    }

    /// Create a physical volume
    fn pv_create(&self, device: &str) -> error::Return {
        utils::command_output(
            "pvcreate",
            &[
                "-y",
                "-ff",
                device,
            ])?;

        log::info!("Physical volume created on `{}`", device);

        return Success!();
    }

    /// Create a volume group
    fn vg_create(&self, device: &str, label: &str) -> error::Return {
        utils::command_output(
            "vgcreate",
            &[
                &format!("vg-{}", label),
                device,
            ])?;

        log::info!("Volume group created on `{}`", device);

        return Success!();
    }

    /// Create logical volumes
    fn volumes_create(&mut self, partition_label: &str) -> error::Return {
        for volume in self.volumes.iter_mut() {
            volume.create(partition_label)?;
        }

        return Success!();
    }
}

impl Validate for Lvm {
    fn is_valid(&self) -> bool {
        return !self.volumes.is_empty();
    }
}

impl Openable for Lvm {
    fn open(&mut self, _passphrase: &str) -> error::Return {
        if self.opened {
            return Success!();
        }

        utils::command_output(
            "vgchange",
            &[
                "-a", "y",
                &format!("vg-{}", self.partition_label),
            ])?;

        log::info!("LVM opened");

        self.opened = true;

        return Success!();
    }

    fn close(&mut self) -> error::Return {
        if !self.opened {
            return Success!();
        }

        utils::command_output(
            "vgchange",
            &[
                "-a", "n",
                &format!("vg-{}", self.partition_label),
            ])?;

        log::info!("LVM closed");

        self.opened = false;

        return Success!();
    }
}

// -----------------------------------------------------------------------------

/// Logical volume structure
#[derive(Debug)]
pub struct Volume {
    /// Json configuration
    pub config: Config,

    /// Whether it's mounted or not
    pub mounted: bool,
}

impl Volume {
    /// Create the logicial volume
    pub fn create(&mut self, partition_label: &str) -> error::Return {
        let opt_size = match self.config.size.is_null() {
            false => "-L",
            true => "-l",
        };

        let size = match self.config.size.is_null() {
            false => self.config.size.to_string(),
            true => "100%FREE".to_string(),
        };

        // Create name of the logical volume
        let vg = format!("vg-{}", partition_label);

        utils::command_output(
            "lvcreate",
            &[
                opt_size, &size,
                "-n", &self.config.label,
                &vg,
            ])?;

        self.config.device = Some(format!("/dev/{}/{}", vg, self.config.label));

        log::info!(
            "Logical volume created: `{}`",
            self.config.device.as_ref().unwrap());

        return Success!();
    }

    /// Format logical volume
    pub fn format(&self) -> error::Return {
        let device = match &self.config.device {
            Some(d) => d,
            None => return generic_error!("No volume device"),
        };

        return gpt::format_partition(
            &device,
            &self.config.fs_type,
            &self.config.label);
    }
}

impl Configurable<Config> for Volume {
    fn from_config(config: &Config) -> Self {
        Self {
            config: config.clone(),
            mounted: false,
        }
    }

    fn config(&self) -> Result<Config, error::Error> {
        return Ok(self.config.clone());
    }
}

impl Mountable for Volume {
    /// Mount logical volume
    fn mount(&mut self, mountpoint: &path::PathBuf) -> error::Return {
        if self.mounted {
            return Success!();
        }

        let device = match &self.config.device {
            Some(d) => d,
            None => return generic_error!("No device for volume"),
        };

        let mountpoint = match mountpoint.to_str() {
            Some(m) => m,
            None => return generic_error!("No mountpoint"),
        };

        utils::command_output("mount", &[device, mountpoint])?;

        self.mounted = true;

        log::info!("`{}` mounted to `{}`", device, mountpoint);

        return Success!();
    }

    /// Unmount logical volume
    fn unmount(&mut self) -> error::Return {
        if !self.mounted {
            return Success!();
        }

        let device = match &self.config.device {
            Some(d) => d,
            None => return generic_error!("No device for volume"),
        };

        utils::command_output("umount", &[device])?;

        self.mounted = false;

        log::info!("`{}` unmounted", device);

        return Success!();
    }
}
