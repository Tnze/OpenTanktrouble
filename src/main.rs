#![windows_subsystem = "windows"]

use open_tanktrouble::run;

fn main() {
    // Init logger
    #[cfg(not(target_arch = "wasm32"))]
        env_logger::init();

    run();
}
