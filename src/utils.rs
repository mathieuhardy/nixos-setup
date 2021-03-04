// -----------------------------------------------------------------------------

use serde::{Serialize};
use std::env;
use std::fs;
use std::io::BufReader;
use std::io::Write;
use std::path;
use std::process;
use std::str;

use super::error;

// -----------------------------------------------------------------------------

/// Write bytes to a file
pub fn write_to_file(content: &[u8], filepath: &path::Path) -> error::Return {
    let mut file = match fs::File::create(filepath) {
        Ok(f) => f,
        Err(e) => return fs_error!(filepath.to_path_buf(), e),
    };

    match file.write_all(content) {
        Ok(_) => return Success!(),
        Err(e) => return fs_error!(filepath.to_path_buf(), e),
    }
}

/// Convert Json object to a printable string
pub fn json_to_string(data: &impl Serialize) -> Result<String, error::Error> {
    let buf = Vec::new();

    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");

    let mut serializer = serde_json::Serializer::with_formatter(
        buf,
        formatter);

    match data.serialize(&mut serializer) {
        Ok(_) => (),
        Err(e) => return json_error!("Cannot serialize data", e),
    }

    match String::from_utf8(serializer.into_inner()) {
        Ok(s) => return Ok(s),
        Err(_) => return generic_error!("Cannot get serializer data"),
    }
}

/// Load Json data from file
pub fn load_json<T>(filepath : &path::Path) -> Result<T, error::Error>
    where
        T: serde::de::DeserializeOwned {

    // Open the file in read-only mode
    let file = match fs::File::open(&filepath) {
        Ok(f) => f,
        Err(e) => return fs_error!(filepath.to_path_buf(), e)
    };

    let reader = BufReader::new(file);

    // Read the JSON contents of the file
    match serde_json::from_reader(reader) {
        Ok(c) => return Ok(c),
        Err(e) => return json_error!(
            filepath.to_path_buf().to_str().unwrap(),
            e)
    };
}
/// Get current directory path
pub fn current_dir() -> Result<path::PathBuf, error::Error> {
    match env::current_dir() {
        Ok(d) => return Ok(d),
        Err(e) => return io_error!("env::current_dir()", e),
    }
}

/// Get output of a command
pub fn command_output(command: &str, args: &[&str])
    -> Result<process::Output, error::Error> {

    log::debug!("Running command: {} {:?}", command, args);

    let output = match process::Command::new(command).args(args).output() {
        Ok(o) => o,
        Err(e) => return io_error!(&format!("`{}` command", command), e),
    };

    if !output.status.success() {
        return generic_error!(
            &format!("`{}` command returned an error", command));
    }

    return Ok(output);
}

/// Convert command output to string
pub fn command_stdout_to_string(output: &process::Output)
    -> Result<String, error::Error> {

    match String::from_utf8(output.stdout.clone()) {
        Ok(s) => return Ok(s),
        Err(e) => return generic_error!(
                &format!("Cannot convert stdout to string: {}", e)),
    }
}

/// Spawn a command with stdout and stderr in pipes
pub fn spawn_command(command: &str, args: &[&str], stdin: Option<&[u8]>)
    -> Result<process::Output, error::Error> {

    log::debug!("Running command: {} {:?}", command, args);

    // Create process
    let mut process = match process::Command::new(command)
        .args(args)
        .stdin(process::Stdio::piped())
        .spawn() {
            Ok(p) => p,
            Err(e) => return cmd_error!(&command, e),
        };

    // Inject stdin if needed
    match stdin {
        Some(s) => {
            log::debug!("...with input: `{}`", str::from_utf8(s).unwrap());

            let mut stream = match process.stdin.take() {
                Some(s) => s,
                None => return generic_error!("Cannot obtain access to stdin"),
            };

            match stream.write_all(s) {
                Ok(_) => (),
                Err(_) => {
                    return generic_error!("Cannot write passphrase to stdin");
                },
            }

            drop(stream);
        },

        None => (),
    }

    // Wait for process to finish
    let output = match process.wait_with_output() {
        Ok(o) => o,
        Err(e) => return io_error!(&format!("`{}` command", command), e),
    };

    if !output.status.success() {
        return generic_error!(
            &format!("`{}` command returned an error", command));
    }

    return Ok(output);
}
