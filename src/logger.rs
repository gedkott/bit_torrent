use std::fs::File;
use std::io::Write;

pub struct Logger {
    file: File,
}

impl Logger {
    pub fn new(filename: &str) -> Self {
        let file = File::create(filename);
        match file {
            Ok(file) => Logger { file },
            Err(e) => {
                panic!("could not open file for logging... {}", e);
            }
        }
    }

    pub fn log(&mut self, s: &str) -> Result<(), std::io::Error> {
        let _ = self.file.write_all(s.as_bytes());
        self.file.write_all(b"\n")
    }
}
