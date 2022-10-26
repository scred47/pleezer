use std::{fs, io};
use toml;

pub fn check(arl_file: &str) -> io::Result<()> {
    // Prevent out-of-memory condition: `arl` file should be small.
    let attributes = fs::metadata(arl_file)?;
    let file_size = attributes.len();

    if file_size > 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "{arl_file} is too large",
        ));
    }

    Ok(())
}

pub fn load(arl_file: &str) -> io::Result<String> {
    check(arl_file)?;

    let contents = fs::read_to_string(arl_file)?;
    let value = contents.parse::<toml::Value>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{arl_file} format is invalid: {e}"),
        )
    })?;

    match value["arl"].as_str() {
        Some(arl) => {
            let chars = arl.chars().count();
            if chars != 192 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("arl should be 192 characters long but is {chars}"),
                ));
            }

            Ok(arl.to_string())
        }
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "{arl_file} does not contain an arl",
        )),
    }
}
