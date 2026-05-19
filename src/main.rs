use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: wav <file.wav>");
        std::process::exit(1);
    }

    let path = &args[1];
    let mut file = File::open(path).unwrap_or_else(|e| {
        eprintln!("Failed to open '{}': {}", path, e);
        std::process::exit(1);
    });

    let (format, mut reader) = wave_rs::parser::parse_file(&mut file).unwrap_or_else(|e| {
        eprintln!("Failed to parse '{}': {}", path, e);
        std::process::exit(1);
    });

    println!(
        "Playing '{}' — {}ch, {}Hz, {}bit",
        path, format.channel, format.sample_rate, format.bits_per_sample
    );

    wave_rs::win32::play_audio(&format, &mut reader, &mut file);
}
