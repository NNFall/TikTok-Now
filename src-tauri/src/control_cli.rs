fn main() {
    #[cfg(windows)]
    std::process::exit(tiktok_now_lib::control_transport::run_cli(&std::env::args().skip(1).collect::<Vec<_>>()));
    #[cfg(not(windows))]
    {
        eprintln!("Local control is available only in the Windows build.");
        std::process::exit(2);
    }
}
