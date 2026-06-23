use std::io::Cursor;

use lewton::inside_ogg::OggStreamReader;

use crate::error::SoundFontError;
use crate::sample_header::SampleHeader;

/// Decodes the concatenated Ogg Vorbis sample streams of a SoundFont3 into a
/// single shared 16-bit PCM buffer and rewrites the sample headers so that they
/// reference the decoded data, matching the layout used by SoundFont2.
///
/// In a SoundFont3, `start`/`end` of each sample header are byte offsets into
/// the compressed `smpl` chunk delimiting that sample's Ogg Vorbis stream, while
/// `start_loop`/`end_loop` are sample-frame offsets relative to the start of the
/// decompressed sample. After decoding, all four become absolute indices into
/// the returned PCM buffer.
pub(crate) fn decode_vorbis_samples(
    vorbis_data: &[u8],
    sample_headers: &mut [SampleHeader],
) -> Result<Vec<i16>, SoundFontError> {
    let mut wave_data: Vec<i16> = Vec::new();

    for header in sample_headers.iter_mut() {
        if header.start < 0 || header.end < header.start || header.end as usize > vorbis_data.len() {
            return Err(SoundFontError::SampleDecompressionFailed(
                "the compressed sample offset is out of range".to_string(),
            ));
        }

        let stream = &vorbis_data[header.start as usize..header.end as usize];
        let decoded = decode_stream(stream)?;

        let base = wave_data.len() as i32;
        let length = decoded.len() as i32;
        wave_data.extend_from_slice(&decoded);

        header.start = base;
        header.end = base + length;
        header.start_loop += base;
        header.end_loop += base;
    }

    // A trailing guard sample lets the oscillator safely read `data[end]` when
    // interpolating the final sample of the buffer.
    wave_data.push(0);

    Ok(wave_data)
}

fn decode_stream(data: &[u8]) -> Result<Vec<i16>, SoundFontError> {
    let mut reader = OggStreamReader::new(Cursor::new(data))
        .map_err(|e| SoundFontError::SampleDecompressionFailed(e.to_string()))?;

    let mut samples: Vec<i16> = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|e| SoundFontError::SampleDecompressionFailed(e.to_string()))?
    {
        // SoundFont samples are mono, so the interleaved packet is the channel.
        samples.extend_from_slice(&packet);
    }

    Ok(samples)
}
