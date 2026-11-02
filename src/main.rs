use anyhow::Result;

fn main() -> Result<()> {
    etol::sanitize_env();
    etol::logs::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let line = if args.is_empty() {
        "etol (sin argumentos)".to_string()
    } else {
        format!("etol {}", args.join(" "))
    };
    etol::logs::info("cli", line);

    let result = etol::cli::run();
    if let Err(error) = &result {
        etol::logs::error("cli", format!("{error:#}"));
    }
    result
}
