// -----------------------------------------------------------------------------

use argon2;
use clap;
use std::fs;
use std::path;

use super::env;
use super::error;
use super::traits::{CliCommand, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_ITERATIONS: &str = "iterations";
const ARG_KEY_SIZE: &str = "key-size";
const ARG_OUTPUT: &str = "output";
const ARG_PASSWORD: &str = "password";
const ARG_SALT: &str = "salt";

// -----------------------------------------------------------------------------

/// Command structure for creating luks key file
#[derive(Debug)]
pub struct Command {
    /// Number of iterations of the algorithm
    iterations: u32,

    /// Size in bytes of the key to be generated
    key_size: u32,

    /// Output file
    output: String,

    /// Password to be used to generate the key
    password: String,

    /// Random salt data
    salt: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return
            self.iterations > 0 &&
            self.key_size > 0 &&
            !self.output.is_empty() &&
            !self.password.is_empty() &&
            !self.salt.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "luks";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Create LUKS key file")
            .version(version)
            .author(author)
            // Iterations argument
            .arg(clap::Arg::with_name(ARG_ITERATIONS)
                .long(ARG_ITERATIONS)
                .help("Number of iterations to perform")
                .required(true)
                .takes_value(true))
            // Iterations argument
            .arg(clap::Arg::with_name(ARG_KEY_SIZE)
                .long(ARG_KEY_SIZE)
                .help("Size of the key")
                .takes_value(true))
            // Password argument
            .arg(clap::Arg::with_name(ARG_OUTPUT)
                .long(ARG_OUTPUT)
                .help("Output file where to store the key")
                .takes_value(true))
            // Password argument
            .arg(clap::Arg::with_name(ARG_PASSWORD)
                .long(ARG_PASSWORD)
                .help("Password to be hashed to create a key file")
                .required(true)
                .takes_value(true))
            // Salt argument
            .arg(clap::Arg::with_name(ARG_SALT)
                .long(ARG_SALT)
                .help("File path containing some salt data")
                .required(true)
                .takes_value(true));
    }

    /// Process command line arguments
    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_ITERATIONS => {
                    let value = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_ITERATIONS),
                    };

                    self.iterations = match value.parse::<u32>() {
                        Ok(i) => i,
                        Err(_) => return inval_error!(&ARG_ITERATIONS),
                    };
                },

                &ARG_KEY_SIZE => {
                    let value = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_KEY_SIZE),
                    };

                    self.key_size = match value.parse::<u32>() {
                        Ok(i) => i,
                        Err(_) => return inval_error!(&ARG_KEY_SIZE),
                    };
                },

                &ARG_OUTPUT => {
                    self.output = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_OUTPUT),
                    };
                },

                &ARG_PASSWORD => {
                    self.password = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_PASSWORD),
                    };
                },

                &ARG_SALT => {
                    self.salt = match matches.value_of(arg.0) {
                        Some(s) => s.to_string(),
                        None => return inval_error!(&ARG_SALT),
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

        // Load data from salt file
        let content = match fs::read(&self.salt) {
            Ok(c) => c,
            Err(e) => return io_error!("Cannot read salt data", e),
        };

        // Hash password
        let hash_config = argon2::Config {
            variant: argon2::Variant::Argon2id,
            version: argon2::Version::Version13,
            mem_cost: 65536,
            time_cost: self.iterations,
            thread_mode: argon2::ThreadMode::Parallel,
            lanes: 4,
            secret: &[],
            ad: &[],
            hash_length: self.key_size
        };

        let hash = match argon2::hash_raw(
            self.password.as_bytes(),
            &content,
            &hash_config) {

            Ok(h) => h,
            Err(_) => {
                return generic_error!("Error while trying to hash password");
            },
        };

        // Write to file
        match utils::write_to_file(&hash, path::Path::new(&self.output)) {
            Ok(_) => log::info!("Key file written to {}", &self.output),
            Err(e) => return Err(e),
        }

        return Success!();
     }
}

impl Command {
    /// Create an instance of Command
    pub fn new() -> Self {
        Self {
            iterations: 0,
            key_size: 4096,
            password: "".to_string(),
            salt: "".to_string(),
            output: "".to_string(),
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.output = config.nixos.key_file;

        return Success!();
    }
}

// -----------------------------------------------------------------------------

/// Function used to set LUKS on a device
pub fn format(device : &str, passphrase : &str) -> error::Return {
    //TODO: use luks2 as soon as possible
    utils::spawn_command(
        "cryptsetup",
        &[
            "luksFormat",
            "-c", "aes-xts-plain64",
            "-s", "256",
            "-h", "sha512",
            "--type", "luks1",
            "-q",
            device,
            "-"
        ],
        Some(passphrase.as_bytes()))?;

    log::info!("LUKS setup on device `{}`", device);

    return Success!();
}

/// Function used to add a key file to a LUKS device
pub fn add_key(
    device : &str,
    passphrase : &str,
    key_file : &str) -> error::Return {

    utils::spawn_command(
        "cryptsetup",
        &[
            "luksAddKey",
            device,
            key_file,
            "-"
        ],
        Some(passphrase.as_bytes()))?;

    return Success!();
}

/// Function used to know if a LUKS device is opened
fn is_opened(label: &str) -> bool {
    let output = match utils::command_output(
        "cryptsetup",
        &[
            "status",
            &format!("/dev/mapper/{}", label),
        ]) {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = match utils::command_stdout_to_string(&output) {
        Ok(s) => s,
        Err(_) => return false,
    };

    return match stdout.find("is active") {
        Some(_) => true,
        None => false,
    };
}

/// Function used to open a LUKS device
pub fn open(device : &str, passphrase : &str, label: &str) -> error::Return {
    if is_opened(label) {
        return Success!();
    }

    utils::spawn_command(
        "cryptsetup",
        &[
            "luksOpen",
            device,
            label,
            "-"
        ],
        Some(passphrase.as_bytes()))?;

    log::info!("LUKS `{}` opened", label);

    return Success!();
}

/// Function used to close a LUKS device
pub fn close(label: &str) -> error::Return {
    match utils::command_output(
        "cryptsetup",
        &[
            "luksClose",
            &format!("/dev/mapper/{}", label),
        ]) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    log::info!("LUKS `{}` closed", label);

    return Success!();
}
