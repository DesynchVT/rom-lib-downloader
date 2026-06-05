use std::process::exit;

mod dessh;
mod destui;
pub mod utils;

fn main() {
    check_env();

    let sess = dessh::create_ssh_session();
    let consoles = dessh::list_consoles(&sess);
    destui::main(&sess, consoles).unwrap();
}

fn check_env() {
    let mut missing_envs: Vec<&str> = vec![];
    let mandatory_env_vars = vec![
        "REMOTE_USERNAME",
        "REMOTE_ROM_ROOT_PATH",
        "LOCAL_ROM_ROOT_PATH",
        "SSH_KEY_PATH",
    ];

    for var in mandatory_env_vars {
        if dotenv::var(var).is_err() {
            missing_envs.push(var);
        }
    }
    if dotenv::var("REMOTE_HOSTNAME").is_err() && dotenv::var("REMOTE_IP_ADDRESS").is_err() {
        missing_envs.push("REMOTE_IP_ADDRESS");
        missing_envs.push("REMOTE_HOSTNAME");
    }

    if !missing_envs.is_empty() {
        println!(
            "The following variables are missing from .env: {:?}",
            missing_envs
        );
        exit(1);
    }
}
