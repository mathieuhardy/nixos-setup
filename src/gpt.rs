// -----------------------------------------------------------------------------

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor};
use std::fmt;
use std::str::FromStr;
use std::thread;
use std::time;

use super::error;
use super::utils;
use super::zfs;

// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum SizeUnit {
    Byte,
    Kilo,
    Mega,
    Giga,
    Tera,
    Peta,
}

impl From<&str> for SizeUnit {
    fn from(s: &str) -> Self {
        return match s {
            "B" => SizeUnit::Byte,
            "K" => SizeUnit::Kilo,
            "M" => SizeUnit::Mega,
            "G" => SizeUnit::Giga,
            "T" => SizeUnit::Tera,
            "P" => SizeUnit::Peta,
            _ => SizeUnit::Byte,
        };
    }
}

impl ToString for SizeUnit {
    fn to_string(&self) -> String {
        return match self {
            SizeUnit::Byte => String::from(""),
            SizeUnit::Kilo => String::from("K"),
            SizeUnit::Mega => String::from("M"),
            SizeUnit::Giga => String::from("G"),
            SizeUnit::Tera => String::from("T"),
            SizeUnit::Peta => String::from("P"),
        }
    }
}

// -----------------------------------------------------------------------------

struct BytesizeVisitor;

impl<'de> Visitor<'de> for BytesizeVisitor {
    type Value = Bytesize;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        return write!(formatter, "struct Bytesize");
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where E: serde::de::Error {
            return Ok(Bytesize::from(s));
    }
}

// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Bytesize {
    unit: SizeUnit,
    value: u64,
}

impl Bytesize {
    pub fn is_null(&self) -> bool {
        return self.value == 0;
    }

    fn to_gpt_string(&self) -> String {
        return match self.value {
            0 => "0".to_string(),
            _ => format!("+{}", self.to_string()),
        }
    }
}

impl Serialize for Bytesize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
            return serializer.serialize_str(&self.to_string());
        }
}

impl<'de> Deserialize<'de> for Bytesize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
            return deserializer.deserialize_str(BytesizeVisitor);
        }
}

impl From<&str> for Bytesize {
    fn from(s: &str) -> Self {
        let pattern = r"^([0-9]+)([BKMGTP])*$";

        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return Self::from("0"),
        };

        let captures = match re.captures(s) {
            Some(c) => c,
            None => return Self::from("0"),
        };

        let value = captures.get(1).map_or("", |m| m.as_str());
        let value = match value.parse::<u64>() {
            Ok(v) => v,
            Err(_) => return Self::from("0"),
        };

        let unit = captures.get(2).map_or("", |m| m.as_str());

        return Self {
            value: value,
            unit: SizeUnit::from(unit),
        };
    }
}

impl ToString for Bytesize {
    fn to_string(&self) -> String {
        return match self.value {
            0 => "0".to_string(),
            _ => format!("{}{}", self.value, self.unit.to_string()),
        }
    }
}

// -----------------------------------------------------------------------------

pub enum PartitionType {
    Efi,
    Linux,
}

impl PartitionType {
    pub fn to_gpt_string(&self) -> String {
        return match self {
            PartitionType::Efi => "ef00".to_string(),
            PartitionType::Linux => "8300".to_string(),
        }
    }
}

impl FromStr for PartitionType {
    type Err = error::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        return match input {
            "efi" | "ef00" => Ok(Self::Efi),
            "linux" | "8300" => Ok(Self::Linux),
            _ => generic_error!("Invalid partition type"),
        };
    }
}

impl ToString for PartitionType {
    fn to_string(&self) -> String {
        return match self {
            PartitionType::Efi => String::from("efi"),
            PartitionType::Linux => String::from("linux"),
        };
    }
}

// -----------------------------------------------------------------------------

/// Enumeration of filesystem types
#[derive(PartialEq)]
pub enum FsType {
    Ext4,
    Fat32,
    Zfs,
    Lvm,
    Swap,
}

impl FromStr for FsType {
    type Err = error::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "ext4" => Ok(Self::Ext4),
            "fat32" => Ok(Self::Fat32),
            "zfs" => Ok(Self::Zfs),
            "lvm" => Ok(Self::Lvm),
            "swap" => Ok(Self::Swap),
            _ => generic_error!(&format!("Invalid enum value {}", input)),
        }
    }
}

// -----------------------------------------------------------------------------

/// Wipeout a device
pub fn wipeout(device: &str) -> error::Return {
    utils::command_output("sgdisk", &["-Z", device])?;

    log::info!("`{}` has been wiped out", device);

    return Success!();
}

/// Create a partition
pub fn create_partition(
    device: &str,
    size: &Bytesize,
    partition_type: &PartitionType,
    label: &str) -> error::Return {

    // Create
    utils::command_output(
        "sgdisk",
        &[
            "-n", &format!("0:0:{}", size.to_gpt_string()),
            "-t", &format!("0:{}", partition_type.to_gpt_string()),
            "-c", &format!("0:{}", label),
            &device,
        ])?;

    log::info!("Partition `{}` has been created", label);

    thread::sleep(time::Duration::from_secs(1));

    return Success!();
}

/// Format a partition
pub fn format_partition(
    device: &str,
    format: &str,
    label: &str) -> error::Return {

    let fs_type = FsType::from_str(format)?;

    match fs_type {
        FsType::Fat32 => format_fat32(device, label)?,
        FsType::Ext4 => format_ext4(device, label)?,
        FsType::Zfs => format_zfs(device, label)?,
        FsType::Swap => format_swap(device, label)?,
        _ => return generic_error!("Invalid partition format"),
    }

    thread::sleep(time::Duration::from_secs(1));

    return Success!();
}

/// Format a partition in FAT32
pub fn format_fat32(device: &str, label: &str) -> error::Return {
    utils::command_output(
        "mkfs.fat",
        &[
            "-F", "32",
            "-n", label,
            device,
        ])?;

    log::info!("Partition `{}` has been formatted in fat32", label);

    return Success!();
}

/// Format a partition in EXT4
pub fn format_ext4(device: &str, label: &str) -> error::Return {
    utils::command_output(
        "mkfs.ext4",
        &[
            "-L", label,
            device,
        ])?;

    log::info!("Partition `{}` has been formatted in ext4", label);

    return Success!();
}

/// Format a partition in ZFS
pub fn format_zfs(device: &str, label: &str) -> error::Return {
    zfs::pool_create(label, device)?;

    log::info!("Partition `{}` has been added to zfs pool `{}`", device, label);

    return Success!();
}

/// Format a swap partition
pub fn format_swap(device: &str, label: &str) -> error::Return {
    utils::command_output(
        "mkswap",
        &[
            "-L", label,
            device,
        ])?;

    log::info!("Partition `{}` has been formatted in swap", label);

    return Success!();
}
