use std::io::{UserRWX,fs};

pub fn mkdirp(path: &Path) {
    match fs::mkdir_recursive(path, UserRWX) {
        Err(e) => fail!("{}", e),
        Ok(_) => {},
    }
}

pub fn mv(from: &Path, to: &Path) {
    match fs::rename(from, to) {
        Err(e) => fail!("{}", e),
        Ok(_) => {},
    }
}

pub fn rmrf(path: &Path) {
    match fs::rmdir_recursive(path) {
        Err(e) => fail!("{}", e),
        Ok(_) => {},
    }
}
