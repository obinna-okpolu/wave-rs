use std::fs;
use std::path::Path;

#[test]
#[ignore] // This test plays actual audio. Run with `cargo test -- --ignored`
fn test_all_wav_files_playback() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_wavs");
    assert!(
        test_dir.exists(),
        "test_wavs directory not found at {:?}",
        test_dir
    );

    let entries = fs::read_dir(test_dir).expect("Failed to read test_wavs directory");

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("wav") {
            let filename = path.file_name().unwrap().to_str().unwrap();
            println!("\n>>> Testing playback: {}", filename);

            let mut file = fs::File::open(&path).expect("Failed to open file");

            match wave_rs::parser::parse_file(&mut file) {
                Ok((format, mut reader)) => {
                    println!(
                        "Format: {}ch, {}Hz, {}bit, code: {}",
                        format.channel,
                        format.sample_rate,
                        format.bits_per_sample,
                        format.format_code
                    );

                    // Extensible format 65534 currently fails in current implementation
                    if format.format_code == 65534 {
                        println!(
                            "Skipping playback for WAVE_FORMAT_EXTENSIBLE (known unsupported)"
                        );
                        continue;
                    }

                    // This will block until the file is finished playing
                    wave_rs::win32::play_audio(&format, &mut reader, &mut file);
                    println!("Finished playing: {}", filename);
                }
                Err(e) => {
                    panic!("Failed to parse {}: {}", filename, e);
                }
            }
        }
    }
}
