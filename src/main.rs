use std::process;

fn main() {
    env_logger::init();

    match pop_core::bin() {
        Ok(()) => (),
        Err(err) => {
            eprintln!("pop-core: error: {}", err);
            process::exit(1);
        }
    }
}
