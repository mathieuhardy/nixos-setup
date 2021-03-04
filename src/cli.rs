// -----------------------------------------------------------------------------

use clap;

use super::env;
use super::error;
use super::hardware;
use super::filesystems;
use super::install;
use super::luks;
use super::partitioning;
use super::secrets;
use super::traits::CliCommand;

// -----------------------------------------------------------------------------

type CommandList = Vec<Box<dyn CliCommand>>;

// -----------------------------------------------------------------------------

pub fn parse() -> error::Return {
    let author = "Mathieu H. <mhardy2008@gmail.com>";
    let version = "1.0";

    // Create command line parser
    let mut app = clap::App::new("NixOS setup")
        .version(version)
        .author(author)
        .about("Performs machine setup for installing NixOS");

    // Add commands
    let mut commands = create_commands();

    for c in commands.iter() {
        app = app.subcommand(c.get(version, author));
    }

    // Get and execute command provided
    let command = match app.get_matches().subcommand {
        Some(c) => c,
        None => return generic_error!("No subcommand provided"),
    };

    for c in commands.iter_mut() {
        if command.name.as_str() == c.name() {
            return c.process(&command.matches);
        }
    }

    return generic_error!("Command has not been processed");
}

fn create_commands() -> CommandList {
    let mut commands: CommandList = Vec::new();

    commands.push(Box::new(env::Command::new()));
    commands.push(Box::new(filesystems::Command::new()));
    commands.push(Box::new(hardware::Command::new()));
    commands.push(Box::new(install::Command::new()));
    commands.push(Box::new(luks::Command::new()));
    commands.push(Box::new(partitioning::Command::new()));
    commands.push(Box::new(secrets::Command::new()));

    return commands;
}
