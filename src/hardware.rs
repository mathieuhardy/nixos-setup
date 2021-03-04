// -----------------------------------------------------------------------------

use clap;
use std::fs;
use std::path;

use super::env;
use super::error;
use super::traits::{CliCommand, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_NAME: &str = "name";

// -----------------------------------------------------------------------------

/// Command structure for creating hardware configuration for NixOS
#[derive(Debug)]
pub struct Command {
    /// Name of the hardware
    hardware: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return !self.hardware.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "hardware";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Create hardware configurations")
            .version(version)
            .author(author)
            // Name argument
            .arg(clap::Arg::with_name(ARG_NAME)
                .long(ARG_NAME)
                .help("Hardware name (optional if a .env file is present)")
                .takes_value(true));
    }

    /// Process command line arguments
    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_NAME => {
                    self.hardware = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_NAME),
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

        // Create output path
        let hw_path = utils::current_dir()?.join("hardware");

        match std::fs::create_dir_all(&hw_path) {
            Ok(_) => log::info!("{:?} has been created", hw_path),
            Err(e) => return fs_error!(hw_path, e),
        };

        // Create temporary directory
        let temp_dir = match mktemp::Temp::new_dir() {
            Ok(f) => f.to_path_buf(),
            Err(e) => return io_error!("/tmp", e),
        };

        log::info!("Temporary directory: {:?}", temp_dir);

        // Generate configuration
        let src_file = self.create_configuration(&temp_dir)?;

        // Move hardware configuration
        self.move_configuration(src_file.to_path_buf())?;

        return Success!();
     }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            hardware: String::from(""),
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.hardware = config.nixos.hardware;

        return Success!();
    }

    /// Create hardware configuration file
    fn create_configuration(&self, temp_dir: &std::path::PathBuf)
        -> Result<std::path::PathBuf, error::Error> {

        let output = match temp_dir.to_str() {
            Some(m) => m,
            None => return generic_error!("No output"),
        };

        utils::command_output("nixos-generate-config", &["--root", output])?;

        //TODO: no filesystems
        let filepath = temp_dir
            .join("etc")
            .join("nixos")
            .join("hardware-configuration.nix");

        log::info!("Configuration generated: {:?}", filepath);

        return Ok(filepath);
    }

    /// Move configuration
    fn move_configuration(&self, src: path::PathBuf) -> error::Return {
        let hardware = format!("{}.nix", self.hardware);
        let tokens: Vec<&str> = hardware.split("_").collect();
        let mut path = path::Path::new(".").join("hardware");

        for s in tokens {
            match s.find(".nix") {
                Some(_) => {
                    match fs::create_dir_all(&path) {
                        Ok(_) => (),
                        Err(e) => return io_error!("Error creating directory", e),
                    }

                    path = path.join("-readonly.nix");
                },

                None => {
                    path = path.join(s);
                },
            }
        }

        log::info!("{:?}", path);

        match fs::copy(&src, &path) {
            Ok(_) => log::info!("Configuration copied to: {:?}", path),
            Err(e) => return fs_error!(src, e),
        }

        return Success!();
    }
}
