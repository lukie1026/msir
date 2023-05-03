use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum MyError {
    #[snafu(display("Could not read file {path}"))]
    ConfigFile {
        source: std::io::Error,
        path: String,
    },
    #[snafu(display("wuuuuuuuu"))]
    Nothing { msg: String },
}

#[warn(dead_code)]
fn read_config_file(path: &str) -> Result<String, MyError> {
    std::fs::read_to_string(path).context(ConfigFileSnafu { path })
}

#[warn(dead_code)]
fn read_config_file2() -> Result<String, MyError> {
    Err(MyError::Nothing {
        msg: "hh".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // let ret = read_config_file("aaa");
        match read_config_file("aaa") {
            Ok(s) => println!("Lukie OK {}", s),
            Err(e) => match e {
                MyError::ConfigFile { source, path } => println!("Lukie Err {} {}", source, path),
                MyError::Nothing { msg } => println!("Lukie Err {}", msg),
            },
        }
    }

    #[test]
    fn it_works2() {
        // let ret = read_config_file("aaa");
        match read_config_file2() {
            Ok(s) => println!("Lukie OK {}", s),
            Err(e) => match e {
                MyError::ConfigFile { source, path } => println!("Lukie Err {} {}", source, path),
                MyError::Nothing { msg } => println!("Lukie Err {}", msg),
            },
        }
    }
}
