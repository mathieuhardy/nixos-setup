// -----------------------------------------------------------------------------

use clap;
use std::collections::HashMap;

use super::env;
use super::filesystem;
use super::error;
use super::traits::{CliCommand, Openable, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_DEVICE: &str = "device";
const ARG_HOST: &str = "host";
const ARG_PASSWORD: &str = "password";

// -----------------------------------------------------------------------------

/// Command structure for creating initramfs on generated filesystem
#[derive(Debug)]
pub struct Command {
    /// Name of the host of the machine to setup
    host: String,

    /// Password used to encrypt/decrypt disks with LUKS
    password: String,

    /// Key file used to decrypt disks with LUKS
    key_file: String,

    /// Filesystem description
    fs_config: Option<filesystem::Config>,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return
            !self.host.is_empty() &&
            !self.key_file.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "partitioning";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Create partitions")
            .version(version)
            .author(author)
            // Device argument
            .arg(clap::Arg::with_name(ARG_DEVICE)
                .long(ARG_DEVICE)
                .help("Device mapping (value must be \"NAME=REPLACEMENT\")")
                .multiple(true)
                .takes_value(true))
            // Host argument
            .arg(clap::Arg::with_name(ARG_HOST)
                .long(ARG_HOST)
                .help("Host name (optional if a .env file is present)")
                .takes_value(true))
            // Password argument
            .arg(clap::Arg::with_name(ARG_PASSWORD)
                .long(ARG_PASSWORD)
                .help("Password to be used to create encrypted partitions")
                .required(true)
                .takes_value(true));
    }

    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        let mut device_mapping: HashMap<String, String> = HashMap::new();

        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_DEVICE=> {
                    match matches.value_of(arg.0) {
                        Some(s) => {
                            let split: Vec<&str> = s.split("=").collect();

                            if split.len() != 2 {
                                return inval_error!(&ARG_DEVICE);
                            }

                            device_mapping.insert(
                                split[0].to_string(),
                                split[1].to_string());
                        },

                        None => return inval_error!(&ARG_DEVICE),
                    }
                },

                &ARG_HOST => {
                    self.host = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_HOST),
                    };
                },

                &ARG_PASSWORD => {
                    self.password = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
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

        log::debug!("{:#?}", self);

        // Check validity
        if !self.is_valid() {
            return generic_error!("Invalid configuration");
        }

        // Create filesystem
        let path = utils::current_dir()?
            .join("layouts")
            .join(format!("{}.in.json", self.host));

        let mut fs = filesystem::Filesystem::from_json(&path)?;

        // Give device mapping
        log::debug!("{:#?}", device_mapping);

        fs.set_device_mapping(&device_mapping);

        // Create partitioning
        fs.create(&self.key_file, &self.password)?;
        fs.close()?;

        // Save back to json file
        let path = utils::current_dir()?
            .join("layouts")
            .join(format!("{}.json", self.host));

        fs.to_json(&path)?;

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
            fs_config: None,
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.host = config.nixos.host;
        self.key_file = config.nixos.key_file;

        return Success!();
    }
}
