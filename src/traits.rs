// -----------------------------------------------------------------------------

use clap;
use std::path;

use super::error;

// -----------------------------------------------------------------------------

pub trait Validate {
    fn is_valid(&self) -> bool;
}

// -----------------------------------------------------------------------------

pub trait CliCommand {
    fn name(&self) -> &'static str;

    fn get<'a, 'b>(
        &self,
        version: &'b str,
        author: &'b str) -> clap::App<'a, 'b>;

    fn process(&mut self, matches: &clap::ArgMatches) -> error::Return;
}

// -----------------------------------------------------------------------------

pub trait Mountable {
    fn mount(&mut self, mountpoint: &path::PathBuf) -> error::Return;

    fn unmount(&mut self) -> error::Return;
}

// -----------------------------------------------------------------------------

pub trait Openable {
    fn open(&mut self, passphrase: &str) -> error::Return;

    fn close(&mut self) -> error::Return;
}

// -----------------------------------------------------------------------------

pub trait Configurable<T> {
    fn from_config(config: &T) -> Self;

    fn config(&self) -> Result<T, error::Error>;
}
