use std::{env, process};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut args = env::args().skip(1);
    let command = match args.next() {
        Some(some) => some,
        None => match env::var("SHELL") {
            Ok(some) => some,
            //TODO: pull default shell from /etc/passwd?
            Err(_) => {
                log::error!("no command provided and SHELL not set");
                process::exit(1);
            }
        },
    };

    match pop_core::run(command, args.collect()) {
        Ok(()) => (),
        Err(err) => {
            log::error!("{}", err);
            process::exit(1);
        }
    }
}
