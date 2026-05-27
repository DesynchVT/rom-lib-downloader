mod des_ssh;
mod tui;

fn main() {
    let sess = des_ssh::create_ssh_session();
    let consoles = des_ssh::list_consoles(&sess);
    tui::main(&sess, consoles).unwrap();
}

fn _do_ssh() {
    dotenv::dotenv().ok();
    let sess = des_ssh::create_ssh_session();
    let consoles = des_ssh::list_consoles(&sess);
    for (i, console) in consoles.iter().enumerate() {
        println!("{}: {}", i, console);
    }
    println!("Input the corresponding number to your desired console: ");
    let mut console_index: usize;
    loop {
        let max_num = &consoles.len() - 1;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input = String::from(input.trim());
        console_index = match input.parse() {
            Ok(n) => n,
            Err(_e) => {
                println!("\"{}\" is not a number. Try again.", &input);
                continue;
            }
        };
        if console_index > max_num {
            println!("Please choose a number between 0 and {}", max_num);
            continue;
        }
        break;
    }

    let roms = des_ssh::list_roms(&sess, &consoles[console_index]);
    for (i, rom) in roms.iter().enumerate() {
        println!("{}: {}", i, rom);
    }
    println!("Input the corresponding number to your desired ROM: ");
    let mut rom_index: usize;
    loop {
        let max_num = &roms.len() - 1;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input = String::from(input.trim());
        rom_index = match input.parse() {
            Ok(n) => n,
            Err(_e) => {
                println!("\"{}\" is not a number. Try again.", &input);
                continue;
            }
        };
        if rom_index > max_num {
            println!("Please choose a number between 0 and {}", max_num);
            continue;
        }
        break;
    }

    let selected_console = &consoles[console_index];
    let selected_rom = &roms[rom_index];

    println!(
        "Do you wish to download {}/{}? Press ENTER to continue or CTRL+C to cancel.",
        selected_console, selected_rom
    );

    let mut _waiter = String::new();
    std::io::stdin().read_line(&mut _waiter).unwrap();

    des_ssh::ssh_download(&sess, selected_console, selected_rom);
}
