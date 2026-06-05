use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;

pub fn create_ssh_session() -> ssh2::Session {
    // Check .env is in order
    let remote_hostname = dotenv::var("REMOTE_HOSTNAME");
    let remote_hostname = if let Ok(hostname) = remote_hostname {
        hostname
    } else {
        dotenv::var("REMOTE_IP_ADDRESS")
            .expect("REMOTE_HOSTNAME and REMOTE_IP_ADDRESS not set in .env")
    };
    let username = dotenv::var("REMOTE_USERNAME").expect("REMOTE_USERNAME not set in .env");
    let ssh_key = dotenv::var("SSH_KEY_PATH").expect("SSH_KEY_PATH not set in .env");
    let ssh_key_path = Path::new(&ssh_key);

    // Connect to the local SSH server
    let tcp = TcpStream::connect(format!("{}@{}:22", username, remote_hostname)).unwrap();
    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();

    // Try to authenticate with the first identity in the agent.
    sess.userauth_pubkey_file(&username, None, ssh_key_path, None)
        .unwrap();
    // Make sure we succeeded
    assert!(sess.authenticated());

    sess
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
