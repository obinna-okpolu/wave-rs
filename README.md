# Rust Win32 WAV Player

A lightweight, low-level WAV audio player for Windows implemented in Rust using the Win32 Multimedia WaveOut API.

## Features
- **Format Support**:
  - PCM (16-bit, 24-bit, 32-bit)
  - IEEE Float (32-bit)
  - A-Law & mu-Law (logarithmic compression)

- **Hopefully robust file parsing**: Custom parser that handles multiple `data` chunks and skips metadata (JUNK, LIST, etc.) gracefully.

## Prerequisites

- **Windows OS**: This project uses Windows-specific APIs.
- **Rust**: [Install Rust](https://www.rust-lang.org/tools/install).

## Usage

To play a WAV file, simply pass the path as an argument:

```powershell
cargo run -- path/to/your/audio.wav
```

## Testing
Run tests
```powershell
cargo test --test playback_test -- --ignored --nocapture
```

## Current limitation
Does not support WAVE_FORMAT_EXTENSIBLE formats

## References
- **Kabal, Peter**    [Audio File Format Specifications: WAVE Specifications](https://www.mmsp.ece.mcgill.ca/Documents/AudioFormats/WAVE/WAVE.html)
- **Microsoft Corporation**    [Waveform Audio File Format](https://learn.microsoft.com/en-us/previous-versions/windows/embedded/ms925318(v=msdn.10))
- **Overton, David**    [Playing Audio in Windows using waveOut Interface](https://github.com/Planet-Source-Code/david-overton-playing-audio-in-windows-using-waveout-interface__3-4422)

## License
MIT / Apache 2.0
