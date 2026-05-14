use std::io::{Read, Seek, SeekFrom};
struct Chunk {
    file_position: usize,
    chunk_size: usize,
    reader_pos: usize,
}

impl Chunk {
    fn new(file_position: usize, chunk_size: usize) -> Self {
        Self {
            file_position,
            chunk_size,
            reader_pos: 0,
        }
    }

    fn update_pos(&mut self, new_pos: usize) {
        self.reader_pos = new_pos;
    }
}

/// An object providing access to `data` chunks of a WAV file.
/// Allows buffer reads from file across multiple separate chunks
pub struct BufferReader {
    chunks: Vec<Chunk>,
    curr_chunk_idx: usize,
}

impl BufferReader {
    /// Create new BufferReader struct
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            curr_chunk_idx: 0,
        }
    }

    /// Add a data chunk
    pub fn add_chunk(&mut self, file_pos: usize, chunk_size: usize) {
        self.chunks.push(Chunk::new(file_pos, chunk_size));
    }

    /// Reads data into the provided buffer from the current position across stored chunks.
    /// Returns the number of bytes read.
    pub fn read_next<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        buf: &mut [u8],
    ) -> std::io::Result<usize> {
        let mut total_read = 0;

        while total_read < buf.len() {
            if self.curr_chunk_idx >= self.chunks.len() {
                break;
            }

            let curr_chunk = &mut self.chunks[self.curr_chunk_idx];
            let (chunk_start, chunk_size, chunk_reader_pos) = (
                curr_chunk.file_position,
                curr_chunk.chunk_size,
                curr_chunk.reader_pos,
            );
            let remaining_in_chunk = chunk_size - chunk_reader_pos;

            if remaining_in_chunk == 0 {
                self.curr_chunk_idx += 1;
                continue;
            }

            let to_read = (buf.len() - total_read).min(remaining_in_chunk);

            reader.seek(SeekFrom::Start((chunk_start + chunk_reader_pos) as u64))?;
            let n = reader.read(&mut buf[total_read..total_read + to_read])?;

            if n == 0 {
                break; // Unexpected EOF in the source
            }

            total_read += n;
            curr_chunk.update_pos(chunk_reader_pos + n);

            if curr_chunk.reader_pos >= chunk_size {
                self.curr_chunk_idx += 1;
            }
        }

        Ok(total_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_buffer_reader_new() {
        let reader = BufferReader::new();
        assert_eq!(reader.chunks.len(), 0);
        assert_eq!(reader.curr_chunk_idx, 0);
    }

    #[test]
    fn test_buffer_reader_add_chunk() {
        let mut reader = BufferReader::new();
        reader.add_chunk(10, 100);
        assert_eq!(reader.chunks.len(), 1);
        assert_eq!(reader.chunks[0].file_position, 10);
        assert_eq!(reader.chunks[0].chunk_size, 100);
        assert_eq!(reader.chunks[0].reader_pos, 0);
    }

    #[test]
    fn test_read_next_single_chunk() {
        let data = b"Hello, World!";
        let mut cursor = Cursor::new(data);
        let mut reader = BufferReader::new();
        reader.add_chunk(0, 13);

        let mut buf = [0u8; 5];
        let n = reader.read_next(&mut cursor, &mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, b"Hello");
        assert_eq!(reader.chunks[0].reader_pos, 5);

        let mut buf2 = [0u8; 10];
        let n2 = reader.read_next(&mut cursor, &mut buf2).unwrap();
        assert_eq!(n2, 8);
        assert_eq!(&buf2[..8], b", World!");
        assert_eq!(reader.chunks[0].reader_pos, 13);
        assert_eq!(reader.curr_chunk_idx, 1);
    }

    #[test]
    fn test_read_next_spanning_chunks() {
        let data = b"Part1Part2Part3";
        let mut cursor = Cursor::new(data);
        let mut reader = BufferReader::new();
        reader.add_chunk(0, 5); // "Part1"
        reader.add_chunk(5, 5); // "Part2"
        reader.add_chunk(10, 5); // "Part3"

        let mut buf = [0u8; 12];
        let n = reader.read_next(&mut cursor, &mut buf).unwrap();
        assert_eq!(n, 12);
        assert_eq!(&buf, b"Part1Part2Pa");
        assert_eq!(reader.curr_chunk_idx, 2);
        assert_eq!(reader.chunks[2].reader_pos, 2);
    }

    #[test]
    fn test_read_next_eof() {
        let data = b"Data";
        let mut cursor = Cursor::new(data);
        let mut reader = BufferReader::new();
        reader.add_chunk(0, 4);

        let mut buf = [0u8; 10];
        let n = reader.read_next(&mut cursor, &mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf[..4], b"Data");

        let n2 = reader.read_next(&mut cursor, &mut buf).unwrap();
        assert_eq!(n2, 0);
    }
}
