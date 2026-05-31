mod des_ssh;
mod tui;

fn main() {
    let sess = des_ssh::create_ssh_session();
    let consoles = des_ssh::list_consoles(&sess);
    tui::main(&sess, consoles).unwrap();
}
