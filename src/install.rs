// -----------------------------------------------------------------------------

use clap;
use std::fs;
use std::os::unix;
use std::path;
use std::thread;
use std::time;

use super::env;
use super::filesystem;
use super::error;
use super::traits::{CliCommand, Openable, Validate};
use super::utils;

// -----------------------------------------------------------------------------

const ARG_HOST: &str = "host";
const ARG_PASSWORD: &str = "password";
const ARG_REPO: &str = "repository";

// -----------------------------------------------------------------------------

/// Command structure for installing NixOS
#[derive(Debug)]
pub struct Command {
    /// Host name
    host: String,

    /// Password used to decrypt disks
    password: String,

    /// Path of the NixOS directory or repository
    repo: String,

    /// Key file to install
    key_file: String,
}

impl Validate for Command {
    fn is_valid(&self) -> bool {
        return
            !self.host.is_empty() &&
            !self.repo.is_empty() &&
            !self.key_file.is_empty();
    }
}

impl CliCommand for Command {
    /// Get the name of the command
    fn name(&self) -> &'static str {
        return "install";
    }

    /// Get command and its arguments
    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b> {

        return clap::App::new(self.name())
            .about("Install NixOS")
            .version(version)
            .author(author)
            // Host argument
            .arg(clap::Arg::with_name(ARG_HOST)
                .long(ARG_HOST)
                .help("Host name (optional if a .env file is present)")
                .takes_value(true))
            // Password argument
            .arg(clap::Arg::with_name(ARG_PASSWORD)
                .long(ARG_PASSWORD)
                .help("Password used to decrypt filesystems")
                .takes_value(true))
            // Repo argument
            .arg(clap::Arg::with_name(ARG_REPO)
                .long(ARG_REPO)
                .help("Path to the NixOS configuration directory or repository")
                .required(true)
                .takes_value(true));
    }

    /// Process command line arguments
    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return {
        // Parse arguments
        for arg in matches.args.iter() {
            match arg.0 {
                &ARG_HOST => {
                    self.host = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_HOST),
                    };
                },

                &ARG_PASSWORD => {
                    self.password = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_PASSWORD),
                    };
                },

                &ARG_REPO => {
                    self.repo = match matches.value_of(arg.0) {
                        Some(s) => s.to_owned(),
                        None => return inval_error!(&ARG_REPO),
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

        // Create filesystem
        let json = utils::current_dir()?
            .join("layouts")
            .join(format!("{}.json", self.host));

        let mut fs = filesystem::Filesystem::from_json(&json)?;

        // Open filesystem
        fs.open(&self.password)?;

        thread::sleep(time::Duration::from_secs(1));

        // Install NixOS
        self.install_nixos(&self.host, &self.repo, &mut fs)?;

        // Close filesystem
        fs.close()?;

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
            repo: "".to_string(),
        }
    }

    /// Use environment file to get needed values
    fn fill_with_env(&mut self) -> error::Return {
        let config = env::read()?;

        self.host = config.nixos.host;
        self.key_file = config.nixos.key_file;

        return Success!();
    }

    /// Install NixOS
    fn install_nixos(
        &self,
        host: &str,
        repo: &str,
        fs: &mut filesystem::Filesystem) -> error::Return {

        // Create paths
        let root = path::Path::new("/").join("mnt").join("root");
        let efi = root.join("boot").join("efi");
        let etc = root.join("etc");

        match fs::create_dir_all(&root) {
            Ok(_) => log::info!("`{:?}` created", root),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // Root partition
        fs.find_system_disk()?.find_root_partition()?.mount(&root)?;

        match fs::create_dir_all(&etc) {
            Ok(_) => log::info!("`{:?}` created", etc),
            Err(e) => return io_error!("Error creating directory", e),
        }

        // EFI partition
        match fs::create_dir_all(&efi) {
            Ok(_) => log::info!("`{:?}` created", efi),
            Err(e) => return io_error!("Error creating directory", e),
        }

        fs.find_system_disk()?.find_efi_partition()?.mount(&efi)?;

        // Install NixOS configuration
        self.install_nixos_repository(host, repo, &etc)?;

        // Run installer
        self.run_nixos_installer(&root)?;

        // Unmount partitions
        fs.find_system_disk()?.find_efi_partition()?.unmount()?;
        fs.find_system_disk()?.find_root_partition()?.unmount()?;

        return Success!();
    }

    /// Install NisOS repository
    fn install_nixos_repository(
        &self,
        host: &str,
        repo: &str,
        etc: &path::PathBuf) -> error::Return {

        let dest = match etc.to_str() {
            Some(m) => m,
            None => return generic_error!("No destination"),
        };

        let mut nixos_repository = repo;

        // Check if it's a repository to clone
        if repo.starts_with("https://github.com") {
            let local_repo = "/tmp/repo-nixos";

            log::info!("Cloning {} to {}", repo, local_repo);

            utils::command_output("git", &["clone", repo, local_repo])?;

            log::info!("{} cloned to {}", repo, local_repo);

            nixos_repository = local_repo;
        }

        // Install repository
        utils::command_output("cp", &["-rf", nixos_repository, dest])?;

        log::info!("`{}` installed to `{}`", repo, dest);

        // Symlink the configuration.nix
        let src = path::Path::new("hosts").join(format!("{}.nix", host));

        let link = etc.join("nixos").join("configuration.nix");

        match fs::symlink_metadata(&link) {
            Ok(_) => fs::remove_file(&link).unwrap(),
            Err(_) => (),
        }

        match unix::fs::symlink(&src, &link) {
            Ok(_) => log::info!("`{:?}` -> `{:?}`", link, src),
            Err(_) => return generic_error!("Cannot symlink the configuration"),
        }

        return Success!();
    }

    /// Run NixOS installer
    fn run_nixos_installer(&self, root: &path::PathBuf) -> error::Return {
        let root = match root.to_str() {
            Some(m) => m,
            None => return generic_error!("No root"),
        };

        utils::command_output(
            "nixos-install",
            &[
                "--no-root-passwd",
                "--root", root
            ])?;

        return Success!();
    }
}
