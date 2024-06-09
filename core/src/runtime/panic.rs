use std::fs;
use std::panic;

pub fn init_graceful_panic_handler() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = fs::write("apsak-ng.log", format!("{:#?}", panic_info));

        default_hook(panic_info);
        crate::runtime::abort();
    }));
}

pub fn init_ungraceful_panic_handler() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = fs::write("apsak-ng.log", format!("{:#?}", panic_info));

        default_hook(panic_info);
        println!("Exiting...");
        std::process::exit(1);
    }));
}
