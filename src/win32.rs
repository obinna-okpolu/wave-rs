#![allow(non_camel_case_types)]
use crate::buffer_reader::BufferReader;
use crate::parser::WAVFormat;
use std::ffi::{c_char, c_void};
use std::io::{Read, Seek};

// ─────────────────────────────────────────────
// Win32 type aliases
// ─────────────────────────────────────────────
type HWAVEOUT = *mut c_void;

type UINT = u32;
type WORD = u16;
type DWORD = u32;
type HANDLE = *mut c_void;
type MMRESULT = UINT;
type DWORD_PTR = usize;

// ─────────────────────────────────────────────
// Win32 WAVEFORMATEX structure
// ─────────────────────────────────────────────

#[repr(C)]
#[derive(Debug)]
struct WAVEFORMATEX {
    format_tag: WORD,
    n_channels: WORD,
    n_samples_per_sec: DWORD,
    n_avg_bytes_per_sec: DWORD,
    n_block_align: WORD,
    w_bits_per_sample: WORD,
    extra_size: WORD,
}

#[repr(C)]
struct WAVEHDR {
    lp_data: *mut c_char,
    buffer_length: DWORD,
    bytes_recorded: DWORD,
    user: DWORD_PTR,
    flags: DWORD,
    loops: DWORD,
    lp_next: *mut WAVEHDR,
    reserved: DWORD_PTR,
}

impl WAVEHDR {
    fn new(data_ptr: *mut c_char, buffer_length: DWORD) -> WAVEHDR {
        WAVEHDR {
            lp_data: data_ptr,
            buffer_length,
            bytes_recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            lp_next: std::ptr::null_mut(),
            reserved: 0,
        }
    }
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn waveOutPrepareHeader(hwo: HWAVEOUT, pwh: *mut WAVEHDR, cbwh: UINT) -> MMRESULT;

    fn waveOutOpen(
        phwo: *mut HWAVEOUT,
        uDeviceID: UINT,
        pwfx: *const WAVEFORMATEX,
        dwCallback: DWORD_PTR,
        dwInstance: DWORD_PTR,
        fdwOpen: DWORD,
    ) -> MMRESULT;

    fn waveOutUnprepareHeader(hwo: HWAVEOUT, pwh: *mut WAVEHDR, cbwh: UINT) -> MMRESULT;

    fn waveOutWrite(hwo: HWAVEOUT, pwh: *mut WAVEHDR, cbwh: UINT) -> MMRESULT;

    fn waveOutClose(hwo: HWAVEOUT) -> MMRESULT;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attrs: *mut c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> HANDLE;

    fn WaitForSingleObject(handle: HANDLE, milliseconds: u32) -> u32;

    fn CloseHandle(handle: HANDLE) -> i32;

}

// ─────────────────────────────────────────────
// WaveOut constants
// ─────────────────────────────────────────────
const MMSYSERR_NOERROR: MMRESULT = 0;
const CALLBACK_EVENT: u32 = 0x0005_0000;
const WAVE_MAPPER: UINT = 0xFFFF_FFFF;
const WHDR_DONE: u32 = 0x0000_0001;
const INFINITE: u32 = 0xFFFF_FFFF;

// Buffer size (64KB)
const BUFFER_SIZE: usize = 64 * 1024;

unsafe fn play_streaming<R: Read + Seek>(
    format: WAVEFORMATEX,
    reader: &mut BufferReader,
    file: &mut R,
) {
    unsafe {
        let hdr_size = std::mem::size_of::<WAVEHDR>() as UINT;

        // Create auto-reset event
        let event = CreateEventW(
            std::ptr::null_mut(),
            0, // auto-reset
            0, // initially non-signalled
            std::ptr::null(),
        );
        if event.is_null() {
            panic!("Failed to create event");
        }

        // Open device
        let mut hwo: HWAVEOUT = std::ptr::null_mut();
        let res = waveOutOpen(
            &mut hwo,
            WAVE_MAPPER,
            &format,
            event as usize,
            0,
            CALLBACK_EVENT,
        );
        if res != MMSYSERR_NOERROR {
            CloseHandle(event);
            eprintln!("{:?}", format);
            panic!("Failed to open audio device. Make sure your speakers are connected.");
        }

        // Eat the WOM_OPEN event
        WaitForSingleObject(event, INFINITE);

        // Create buffers & headers
        let buf_size =
            (BUFFER_SIZE / format.n_block_align as usize) * format.n_block_align as usize;
        let mut bufs = [vec![0u8; buf_size], vec![0u8; buf_size]];
        let mut headers = [
            WAVEHDR::new(bufs[0].as_mut_ptr() as *mut c_char, buf_size as u32),
            WAVEHDR::new(bufs[1].as_mut_ptr() as *mut c_char, buf_size as u32),
        ];
        let mut flying = [false; 2];
        let mut eof = false;

        // Fill both buffers and submit them
        for i in 0..2 {
            let n = reader.read_next(file, &mut bufs[i]).unwrap_or(0);
            if n == 0 {
                eof = true;
                break;
            }
            headers[i].lp_data = bufs[i].as_mut_ptr() as *mut c_char;
            headers[i].buffer_length = n as u32;
            waveOutPrepareHeader(hwo, &mut headers[i], hdr_size);
            waveOutWrite(hwo, &mut headers[i], hdr_size);
            flying[i] = true;
        }

        // Playback loop
        loop {
            WaitForSingleObject(event, INFINITE);

            for i in 0..2 {
                if !flying[i] {
                    continue;
                }
                if headers[i].flags & WHDR_DONE == 0 {
                    continue;
                }

                // Buffer i just finished playing
                waveOutUnprepareHeader(hwo, &mut headers[i], hdr_size);
                flying[i] = false;

                if !eof {
                    let n = reader.read_next(file, &mut bufs[i]).unwrap_or(0);
                    if n == 0 {
                        eof = true;
                        continue;
                    }
                    headers[i].lp_data = bufs[i].as_mut_ptr() as *mut c_char;
                    headers[i].buffer_length = n as u32;
                    headers[i].flags = 0;
                    waveOutPrepareHeader(hwo, &mut headers[i], hdr_size);
                    waveOutWrite(hwo, &mut headers[i], hdr_size);
                    flying[i] = true;
                }
            }

            if eof && !flying[0] && !flying[1] {
                break;
            }
        }

        // Cleanup
        waveOutClose(hwo);
        CloseHandle(event);
    }
}

/// Play audio from a parsed WAV file.
pub fn play_audio<R: Read + Seek>(format: &WAVFormat, reader: &mut BufferReader, file: &mut R) {
    let wfx = WAVEFORMATEX {
        format_tag: format.format_code,
        n_channels: format.channel,
        n_samples_per_sec: format.sample_rate,
        n_avg_bytes_per_sec: format.byte_rate,
        n_block_align: format.block_align,
        w_bits_per_sample: format.bits_per_sample,
        extra_size: format.ext_size,
    };

    unsafe {
        play_streaming(wfx, reader, file);
    }
}
