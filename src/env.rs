// -----------------------------------------------------------------------------

use clap;
use serde::{Deserialize, Serialize};
use std::path;

use super::error;
use super::traits::{CliCommand, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_HARDWARE: &str = "hardware";
const ARG_HOST: &str = "host";
const ARG_KEY_FILENAME: &str = "key-name";
const ARG_KEY_FILEPATH: &str = "key-path";
const ARG_WPA_PASSWORD: &str = "wpa-password";
const ARG_WPA_SSID: &str = "wpa-ssid";

// -----------------------------------------------------------------------------

/// Structure reprensenting the hierarchy of the Json file
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Nixos node
    pub nixos: NixOSConfig
}

impl Validate for Config {
    fn is_valid(&self) -> bool {
        return self.nixos.is_valid();
    }
}

// -----------------------------------------------------------------------------

/// Structure representing the `nixos` node in Json file
#[derive(Debug, Serialize, Deserialize)]
pub struct NixOSConfig {
    /// The host name to be setup
    pub host: String,

    /// The hardware name to be setup
    pub hardware: String,

    /// The path of the key file to be used for unlocking disks
    pub key_file: String,

    /// The filename of the key file
    pub key_filename: String,
}

impl Validate for NixOSConfig {
    fn is_valid(&self) -> bool {
        return
            !self.host.is_empty() &&
            !self.hardware.is_empty() &&
            !self.key_file.is_empty() &&
            !self.key_filename.is_empty();
    }
}

// -----------------------------------------------------------------------------

/// Command structure for setting environment
#[derive(Debug)]
pub struct Command {
    /// The SSID of the WiFi network
    wpa_ssid: String,

    /// The password of the WiFi network
    wpa_password: String,

    /// The Json configuration
    config: Config,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return self.config.is_valid();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "env";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Prepare environment (variables, WiFi, ...)")
            .version(version)
            .author(author)
            // Hardware argument
            .arg(clap::Arg::with_name(ARG_HARDWARE)
                .long(ARG_HARDWARE)
                .help("Hardware name")
                .required(true)
                .takes_value(true))
            // Host argument
            .arg(clap::Arg::with_name(ARG_HOST)
                .long(ARG_HOST)
                .help("Host name")
                .required(true)
                .takes_value(true))
            // Key filename argument
            .arg(clap::Arg::with_name(ARG_KEY_FILENAME)
                .long(ARG_KEY_FILENAME)
                .help("Key filename")
                .required(true)
                .takes_value(true))
            // Key filepath argument
            .arg(clap::Arg::with_name(ARG_KEY_FILEPATH)
                .long(ARG_KEY_FILEPATH)
                .help("Key filepath (where the key will be generated)")
                .takes_value(true))
            // WPA password argument
            .arg(clap::Arg::with_name(ARG_WPA_PASSWORD)
                .long(ARG_WPA_PASSWORD)
                .help("WiFi password")
                .takes_value(true))
            // WPA SSID argument
            .arg(clap::Arg::with_name(ARG_WPA_SSID)
                .long(ARG_WPA_SSID)
                .help("WiFi SSID")
                .takes_value(true));
    }

    /// Process command line arguments
    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        let mut key_path = "/tmp".to_string();

        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_HARDWARE => {
                    self.config.nixos.hardware = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_HARDWARE),
                    };
                },

                &ARG_HOST => {
                    self.config.nixos.host = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_HOST),
                    };
                },

                &ARG_KEY_FILENAME => {
                    self.config.nixos.key_filename =
                        match matches.value_of(arg.0) {
                            Some(s) => s.to_string(),
                            None => return inval_error!(&ARG_KEY_FILENAME),
                        };
                },

                &ARG_KEY_FILEPATH => {
                    key_path = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_KEY_FILEPATH),
                    };
                },

                &ARG_WPA_PASSWORD => {
                    self.wpa_password = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_WPA_PASSWORD),
                    };
                },

                &ARG_WPA_SSID => {
                    self.wpa_ssid = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_WPA_SSID),
                    };
                },

                _ => {
                    return inval_error!(arg.0);
                }
            }
        }

        // Create key filepath
        if key_path.is_empty() {
            return inval_error!(&key_path);
        }

        self.config.nixos.key_file = match path::Path::new(&key_path)
            .join(&self.config.nixos.key_filename)
            .to_str() {
                Some(s) => s.to_string(),
                None => return generic_error!("Cannot build key filepath"),
            };

        log::debug!("{:#?}", self);

        // Check validity
        if !self.is_valid() {
            return generic_error!("Invalid configuration");
        }

        // Perform setups
        self.setup_environment()?;
        self.setup_keyboard_layout()?;
        self.setup_wpa_supplicant()?;

        return Success!();
    }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            wpa_ssid: "".to_string(),
            wpa_password: "".to_string(),

            config: Config {
                nixos: NixOSConfig {
                    host: "".to_string(),
                    hardware: "".to_string(),
                    key_file: "".to_string(),
                    key_filename: "".to_string(),
                }
            }
        }
    }

    /// Create an environment file named `.env`, in the current directory, that
    /// contains Json data describing the setup environement.
    fn setup_environment(&self) -> error::Return {
        // Serialize to Json string
        let json = utils::json_to_string(&self.config)?;

        log::debug!("{}", json);

        // Create output path
        let output = utils::current_dir()?.join(".env");

        // Write to file
        utils::write_to_file(json.as_bytes(), &output)?;

        log::info!("NixOS configuration wrote to {:?}", output);

        return Success!();
    }

    /// Setup the keyboard layout to french
    fn setup_keyboard_layout(&self) -> error::Return {
        let output = utils::command_output("loadkeys", &["fr"])?;

        match output.status.success() {
            true => log::info!("Keyboard layout configured"),
            false => return process_error!("loadkeys", output.status),
        }

        return Success!();
    }

    /// Setup WpaSupplicant configuration in order to connect to WiFi
    fn setup_wpa_supplicant(&self) -> error::Return {
        if self.wpa_ssid.is_empty() || self.wpa_password.is_empty() {
            return Success!();
        }

        // Generate configuration
        let output = utils::command_output(
            "wpa_passphrase",
            &[
                &self.wpa_ssid,
                &self.wpa_password,
            ])?;

        if !output.status.success() {
            return process_error!("wpa_passphrase", output.status);
        }

        let stdout = utils::command_stdout_to_string(&output)?;

        log::debug!("{}", stdout);

        // Write to file
        let path = path::Path::new("/").join("etc").join("wpa_supplicant.conf");

        utils::write_to_file(stdout.as_bytes(), &path)?;

        log::info!("WPA configuration written to {:?}", path);

        // Restart WiFi service
        let output = utils::command_output(
            "systemctl",
            &[
                "restart",
                "wpa_supplicant",
            ])?;

        match output.status.success() {
            true => log::info!("WiFi is enabled"),
            false => return process_error!("systemctl", output.status),
        }

        return Success!();
    }
}

// -----------------------------------------------------------------------------

/// Method used to load environment configuraition from Json file `.env`
pub fn read() -> Result<Config, error::Error> {
    let path = utils::current_dir()?.join(".env");

    return utils::load_json(&path);
}
