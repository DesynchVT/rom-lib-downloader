mod dessh;
mod destui;
pub mod utils;

fn main() {
    let sess = dessh::create_ssh_session();
    let consoles = dessh::list_consoles(&sess);
    destui::main(&sess, consoles).unwrap();
}
