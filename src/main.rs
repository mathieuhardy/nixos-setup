// -----------------------------------------------------------------------------

use env_logger;

#[macro_use]
mod error;

mod cli;
mod disk;
mod env;
mod filesystem;
mod filesystems;
mod gpt;
mod hardware;
//mod initramfs;
mod install;
mod luks;
mod lvm;
mod partition;
mod partitioning;
mod secrets;
mod traits;
mod utils;
mod zfs;

// -----------------------------------------------------------------------------

fn main() {
    // Configure logs
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Trace)
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    // Parse command line interface
    match cli::parse() {
        Ok(_) => log::info!("Finished!"),
        Err(e) => log::error!("{}", e)
    }
}
