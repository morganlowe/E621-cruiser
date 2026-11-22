fn main() {
    #[cfg(windows)]
    {
        if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/e621-cruiser.ico");
            if let Err(e) = res.compile() {
                eprintln!("winresource failed: {e}");
                std::process::exit(1);
            }
        }
    }
}
