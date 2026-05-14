use crate::buffer_reader::BufferReader;
use std::io::{Read, Seek, SeekFrom};

// Chunk positions are no longer used as we scan for chunks.
pub struct WAV {
    pub format: WAVFormat,
    pub audio_data: Vec<u8>,
}

pub struct WAVFormat {
    pub format_code: u16,
    pub channel: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub ext_size: u16,
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn get_format(fmt_buf: &[u8]) -> WAVFormat {
    WAVFormat {
        format_code: read_u16_le(fmt_buf, 0),
        channel: read_u16_le(fmt_buf, 2),
        sample_rate: read_u32_le(fmt_buf, 4),
        byte_rate: read_u32_le(fmt_buf, 8),
        block_align: read_u16_le(fmt_buf, 12),
        bits_per_sample: read_u16_le(fmt_buf, 14),
        ext_size: if fmt_buf.len() >= 18 {
            read_u16_le(fmt_buf, 16)
        } else {
            0
        },
    }
}

pub fn parse_file<R: Read + Seek>(file: &mut R) -> Result<(WAVFormat, BufferReader), String> {
    let mut reader = BufferReader::new();
    let mut format: Option<WAVFormat> = None;

    // Header validation
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|_| "Invalid file format. Reached unexpected end of file.".to_string())?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("Invalid file format: Missing RIFF/WAVE header".to_string());
    }

    // Scan for chunks
    loop {
        let mut chunk_header = [0u8; 8];
        let bytes_read = file
            .read(&mut chunk_header)
            .map_err(|_| "Error reading chunk header".to_string())?;

        if bytes_read == 0 {
            break;
        } // EOF

        if bytes_read < 8 {
            return Err("Truncated chunk header".to_string());
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes(chunk_header[4..8].try_into().unwrap()) as usize;

        match chunk_id {
            b"fmt " => {
                let mut fmt_buf = vec![0u8; chunk_size];
                file.read_exact(&mut fmt_buf)
                    .map_err(|_| "Failed to read fmt chunk".to_string())?;

                if chunk_size < 16 {
                    return Err("fmt chunk too small".to_string());
                }

                format = Some(get_format(&fmt_buf));
            }
            b"data" => {
                let current_pos = file
                    .stream_position()
                    .map_err(|_| "Failed to get file position".to_string())?;
                reader.add_chunk(current_pos as usize, chunk_size);

                // Skip the data to find the next chunk
                file.seek(SeekFrom::Current(chunk_size as i64))
                    .map_err(|_| "Failed to seek past data chunk".to_string())?;
            }
            _ => {
                // Skip unknown chunks
                file.seek(SeekFrom::Current(chunk_size as i64))
                    .map_err(|_| "Failed to seek past unknown chunk".to_string())?;
            }
        }
    }

    let fmt = format.ok_or("Missing fmt chunk".to_string())?;
    Ok((fmt, reader))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn create_mock_wav() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes()); // Size (unused)
        data.extend_from_slice(b"WAVE");

        // fmt chunk (16 bytes for PCM)
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
        data.extend_from_slice(&1u16.to_le_bytes()); // Format code (PCM)
        data.extend_from_slice(&2u16.to_le_bytes()); // Channels
        data.extend_from_slice(&44100u32.to_le_bytes()); // Sample rate
        data.extend_from_slice(&176400u32.to_le_bytes()); // Byte rate
        data.extend_from_slice(&4u16.to_le_bytes()); // Block align
        data.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample

        // data chunk
        data.extend_from_slice(b"data");
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&[1u8; 8]); // Mock audio data

        data
    }

    #[test]
    fn test_parse_multiple_data_chunks() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WAVE");

        // fmt chunk
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]); // Dummy fmt data

        // data chunk 1
        data.extend_from_slice(b"data");
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(b"PART");

        // some other chunk to skip
        data.extend_from_slice(b"junk");
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(b"SKIP");

        // data chunk 2
        data.extend_from_slice(b"data");
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(b"SONG");

        let mut cursor = Cursor::new(data);
        let result = parse_file(&mut cursor);
        assert!(result.is_ok());
        let (_, mut reader) = result.unwrap();

        let mut output = [0u8; 8];
        reader.read_next(&mut cursor, &mut output).unwrap();
        assert_eq!(&output, b"PARTSONG");
    }

    #[test]
    fn test_parse_real_wav_files() {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_wavs");
        if !test_dir.exists() {
            return;
        }

        //fmt data extracted using Python's wav library
        let mut ground_truth = std::collections::HashMap::new();
        ground_truth.insert("addf8-Alaw-GW.wav", (6, 1, 8000, 8));
        ground_truth.insert("addf8-mulaw-GW.wav", (7, 1, 8000, 8));
        ground_truth.insert("M1F1-Alaw-AFsp.wav", (6, 2, 8000, 8));
        ground_truth.insert("M1F1-float32-AFsp.wav", (3, 2, 8000, 32));
        ground_truth.insert("M1F1-float32WE-AFsp.wav", (65534, 2, 8000, 32));
        ground_truth.insert("M1F1-int16-AFsp.wav", (1, 2, 8000, 16));
        ground_truth.insert("M1F1-mulaw-AFsp.wav", (7, 2, 8000, 8));

        let entries = std::fs::read_dir(test_dir).expect("Failed to read test_wavs directory");

        for entry in entries {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            let filename = path.file_name().unwrap().to_str().unwrap();

            if let Some(&(expected_code, expected_channels, expected_rate, expected_bits)) =
                ground_truth.get(filename)
            {
                let mut file = std::fs::File::open(&path).expect("Failed to open file");
                let (format, _reader) = parse_file(&mut file).expect("Failed to parse file");

                assert_eq!(
                    format.format_code, expected_code,
                    "Format code mismatch for {}",
                    filename
                );
                assert_eq!(
                    format.channel, expected_channels,
                    "Channel mismatch for {}",
                    filename
                );
                assert_eq!(
                    format.sample_rate, expected_rate,
                    "Sample rate mismatch for {}",
                    filename
                );
                assert_eq!(
                    format.bits_per_sample, expected_bits,
                    "Bits per sample mismatch for {}",
                    filename
                );
            }
        }
    }

    #[test]
    fn test_parse_valid_wav() {
        let wav_data = create_mock_wav();
        let mut cursor = Cursor::new(wav_data);
        let result = parse_file(&mut cursor);
        assert!(result.is_ok());
        let (format, _reader) = result.unwrap();
        assert_eq!(format.format_code, 1);
        assert_eq!(format.channel, 2);
        assert_eq!(format.sample_rate, 44100);
    }

    #[test]
    fn test_parse_truncated_chunk() {
        let mut data = create_mock_wav();
        let original_len = data.len();
        data.truncate(original_len - 4); // Truncate the data chunk content (half of the 8 bytes)

        let mut cursor = Cursor::new(data);
        let result = parse_file(&mut cursor);
        assert!(result.is_ok());
        let (_, mut reader) = result.unwrap();

        let mut buf = [0u8; 8];
        let n = reader.read_next(&mut cursor, &mut buf).unwrap();
        assert_eq!(n, 4); // Only 4 bytes should be available
    }

    #[test]
    fn test_parse_invalid_header() {
        let data = b"NOT_A_WAVE_FILE_AT_ALL";
        let mut cursor = Cursor::new(data);
        let result = parse_file(&mut cursor);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Invalid file format: Missing RIFF/WAVE header"
        );
    }

    #[test]
    fn test_parse_missing_fmt() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"data");
        data.extend_from_slice(&0u32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let result = parse_file(&mut cursor);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Missing fmt chunk");
    }
}
