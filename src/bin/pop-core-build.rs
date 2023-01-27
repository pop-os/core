use std::process;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    match pop_core::build() {
        Ok(()) => (),
        Err(err) => {
            eprintln!("pop-core: error: {}", err);
            process::exit(1);
        }
    }
}
