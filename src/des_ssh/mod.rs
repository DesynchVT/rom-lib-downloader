use indicatif::{ProgressBar, ProgressStyle};
use ssh2::Session;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use zip::ZipArchive;

pub fn unzip(zip_path: &str, dest_dir: &str) {
    let file = File::open(zip_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    archive.extract(dest_dir).unwrap();
}

pub fn create_ssh_session() -> ssh2::Session {
    // Connect to the local SSH server
    let tcp = TcpStream::connect("homelab:22").unwrap();
    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();

    // Try to authenticate with the first identity in the agent.
    let username = dotenv::var("REMOTE_USERNAME").expect("REMOTE_USERNAME not set in .env");
    let ssh_key = dotenv::var("SSH_KEY_PATH").expect("SSH_KEY_PATH not set in .env");
    let ssh_key_path = Path::new(&ssh_key);

    sess.userauth_pubkey_file(&username, None, ssh_key_path, None)
        .unwrap();
    // Make sure we succeeded
    assert!(sess.authenticated());

    sess
}

pub fn ssh_download(sess: &ssh2::Session, console: &str, rom: &str) {
    let local_root_path =
        dotenv::var("LOCAL_ROM_ROOT_PATH").expect("LOCAL_ROM_ROOT_PATH not set in .env");
    let remote_root_path =
        dotenv::var("REMOTE_ROM_ROOT_PATH").expect("REMOTE_ROM_ROOT_PATH not set in .env");

    let rom_wo_extension = &rom[..rom.rfind('.').unwrap_or(rom.len())];
    let dest_path = format!("{}/{}/{}", local_root_path, console, rom_wo_extension);

    let rom_path = format!("/{}/{}", console, rom);
    let remote_rom_path = format!("{}/{}", remote_root_path, rom_path);
    let local_console_path = format!("{}/{}", local_root_path, console);
    let temp_download_dir = format!("{}/.downloading", local_console_path);
    let part_path = format!("{}/{}.part", temp_download_dir, rom);

    // Connect to the local SSH server
    let (mut remote_file, stat) = sess.scp_recv(Path::new(&remote_rom_path)).unwrap();
    println!("remote file size: {}", stat.size());

    // Show a progress bar while downloading
    let pb = ProgressBar::new(stat.size());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap(),
    );

    // Download the file via scp
    let mut buf = vec![0u8; 8192];

    // Open file before the loop, write chunks as they arrive, rename at the end
    fs::create_dir_all(&temp_download_dir).unwrap();
    let mut local_file = fs::File::create(&part_path).unwrap();
    loop {
        let n = remote_file.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        local_file.write_all(&buf[..n]).unwrap();
        pb.inc(n as u64);
    }
    // Update the progress bar
    pb.finish_with_message("Downloaded");

    let completed_download = format!("{}/{}", temp_download_dir, rom_wo_extension);

    unzip(&part_path, &completed_download);
    fs::rename(&completed_download, &dest_path).unwrap();

    // Optional: delete the zip after extraction
    fs::remove_file(&part_path).unwrap();
    fs::remove_dir_all(temp_download_dir).unwrap();

    // Close the channel and wait for the whole content to be transferred
    remote_file.send_eof().unwrap();
    remote_file.wait_eof().unwrap();
    remote_file.close().unwrap();
    remote_file.wait_close().unwrap();
}

fn do_ls(sess: &ssh2::Session, console: Option<String>) -> Vec<String> {
    let remote_root_path =
        dotenv::var("REMOTE_ROM_ROOT_PATH").expect("REMOTE_ROM_ROOT_PATH not set in .env");

    let target_dir = if let Some(c) = console {
        &format!("{}/{}", &remote_root_path, c)
    } else {
        &remote_root_path
    };

    let mut channel = sess.channel_session().unwrap();

    let cmd = format!("ls {}", target_dir);
    channel.exec(&cmd).unwrap();

    let mut dir_list_str = String::new();
    channel.read_to_string(&mut dir_list_str).unwrap();
    channel.wait_close().unwrap();
    let list: Vec<String> = dir_list_str.lines().map(String::from).collect();
    list
}

pub fn list_consoles(sess: &ssh2::Session) -> Vec<String> {
    do_ls(sess, None)
}

pub fn list_roms(sess: &ssh2::Session, console: &str) -> Vec<String> {
    do_ls(sess, Some(String::from(console)))
}
