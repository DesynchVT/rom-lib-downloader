use std::fs::File;
use zip::ZipArchive;

pub fn bytes_to_human_readable(bytes: u64) -> String {
    let bytes = bytes as f64;
    let megabytes = bytes / 1_048_576.0;

    if megabytes >= 1024.0 {
        let gigabytes = bytes / 1_073_741_824.0;
        format!("{:.2}GB", gigabytes)
    } else {
        format!("{:.2}MB", megabytes)
    }
}

pub fn unzip(zip_path: &str, dest_dir: &str) {
    let file = File::open(zip_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    archive.extract(dest_dir).unwrap();
}
