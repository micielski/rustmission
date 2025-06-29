use std::{
    fs,
    path::{Path, PathBuf},
};

use zune_inflate::DeflateDecoder;

#[derive(Debug)]
struct Manual {
    path: PathBuf,
}

impl Manual {
    fn read_raw(&self) -> String {
        let raw_bytes = fs::read(&self.path).unwrap();
        let mut decoder = DeflateDecoder::new(&raw_bytes);
        let decompressed_manual = decoder.decode_gzip().unwrap();

        String::from_utf8(decompressed_manual).unwrap()
    }

    fn read(&self) {
        let raw_content = self.read_raw();

        let mut date = None;

        for line in raw_content.lines() {
            if line.starts_with(".Dd") {
                date = Some(line[4..].to_string());
            }
        }
    }

    fn get_all_of_category(category: u8) -> Vec<Manual> {
        let paths = fs::read_dir(format!("/usr/share/man/man{category}")).unwrap();
        let mut manuals_paths = vec![];
        for path in paths {
            let path = path.unwrap();
            if path.file_type().unwrap().is_file() {
                manuals_paths.push(path.path());
            }
        }

        let mut manuals = vec![];
        for manual_path in manuals_paths {
            manuals.push(Manual { path: manual_path });
        }
        manuals
    }
}

fn main() {
    let test_manuals = Manual::get_all_of_category(8);
    dbg!(test_manuals);
}
