// Renders a sneaky little caper tune using a SoundFont3
// (.sf3) file, whose samples are Ogg Vorbis compressed. Requires the `sf3`
// feature:
//
//     cargo run -p example --features sf3 --example sf3
//
// The rendered audio is written to `sf3.wav` next to the soundfont.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use rustysynth::SoundFont;
use rustysynth::Synthesizer;
use rustysynth::SynthesizerSettings;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("samples")
}

// MIDI channels.
const MELODY: i32 = 0; // celesta
const BASS: i32 = 1; // pizzicato strings
const CHORDS: i32 = 2; // vibraphone
const DRUMS: i32 = 9; // GM percussion

// A note placed on the 16th-note grid: (start_step, length_steps, channel, key, velocity).
type Note = (f64, f64, i32, i32, i32);

fn main() {
    let sf3_path = samples_dir().join("FluidR3Mono_GM.sf3");

    // Load the compressed SoundFont3.
    let mut file = File::open(&sf3_path).unwrap();
    let sound_font = Arc::new(SoundFont::new(&mut file).unwrap());

    let info = sound_font.get_info();
    println!("Loaded SoundFont: {}", info.get_bank_name());
    println!(
        "  version: {}.{}",
        info.get_version().get_major(),
        info.get_version().get_minor()
    );
    let settings = SynthesizerSettings::new(44100);
    let mut synthesizer = Synthesizer::new(&sound_font, &settings).unwrap();

    // Pick instruments (GM program changes; command 0xC0).
    synthesizer.process_midi_message(MELODY, 0xC0, 8, 0); // Celesta
    synthesizer.process_midi_message(BASS, 0xC0, 45, 0); // Pizzicato Strings
    synthesizer.process_midi_message(CHORDS, 0xC0, 11, 0); // Vibraphone

    let notes = compose();

    // Turn the notes into time-ordered note-on / note-off events.
    let bpm = 132.0;
    let step = 60.0 / bpm / 4.0; // seconds per 16th note
    let sr = settings.sample_rate as f64;
    let to_sample = |t: f64| (t * step * sr).round() as usize;

    let mut events: Vec<(usize, i32, i32, i32)> = Vec::new();
    let mut last = 0usize;
    for &(start, len, ch, key, vel) in &notes {
        let on = to_sample(start);
        let off = to_sample(start + len);
        events.push((on, ch, key, vel));
        events.push((off, ch, key, 0)); // velocity 0 == note off
        last = last.max(off);
    }
    events.sort_by_key(|e| e.0);

    // Render, advancing the synthesizer up to each event in turn.
    let tail = (2.5 * sr) as usize; // let the final chord ring out
    let total = last + tail;
    let mut left = vec![0_f32; total];
    let mut right = vec![0_f32; total];

    let mut pos = 0usize;
    for (sample, ch, key, vel) in events {
        let target = sample.min(total);
        if target > pos {
            synthesizer.render(&mut left[pos..target], &mut right[pos..target]);
            pos = target;
        }
        if vel > 0 {
            synthesizer.note_on(ch, key, vel);
        } else {
            synthesizer.note_off(ch, key);
        }
    }
    if pos < total {
        synthesizer.render(&mut left[pos..], &mut right[pos..]);
    }

    let out_path = samples_dir().join("sf3.wav");
    write_wav(&left, &right, settings.sample_rate as u32, &out_path);
    println!(
        "Rendered {:.1}s. Wrote {}",
        total as f64 / sr,
        out_path.display()
    );
}

/// Builds the whole arrangement on a 16th-note grid (16 steps per bar).
fn compose() -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    let mut push = |start: usize, len: f64, ch: i32, key: i32, vel: i32| {
        notes.push((start as f64, len, ch, key, vel));
    };

    // Chord progression: Em - C - Am - B7, looped twice (8 bars).
    let roots = [40, 48, 45, 47]; // E2, C3, A2, B2
    let fifths = [47, 43, 52, 54]; // B2, G2, E3, F#3
    let chords: [&[i32]; 4] = [
        &[64, 67, 71],     // Em : E4 G4 B4
        &[60, 64, 67],     // C  : C4 E4 G4
        &[57, 60, 64],     // Am : A3 C4 E4
        &[59, 63, 66, 69], // B7 : B3 D#4 F#4 A4
    ];

    // Celesta melody, one 16-step pattern per chord (0 == rest). Skippy and sly.
    let melody: [[i32; 16]; 4] = [
        // Em
        [0, 0, 76, 0, 78, 0, 79, 0, 0, 0, 83, 0, 81, 79, 78, 0],
        // C
        [79, 0, 0, 0, 81, 0, 83, 0, 84, 0, 83, 81, 79, 0, 0, 0],
        // Am
        [0, 76, 0, 78, 0, 79, 0, 81, 0, 0, 83, 0, 84, 83, 81, 79],
        // B7 (chromatic creep: B A F# D# B D# F# A B)
        [83, 0, 81, 0, 78, 0, 75, 0, 71, 0, 75, 0, 78, 0, 81, 83],
    ];

    for bar in 0..8usize {
        let i = bar % 4;
        let b = bar * 16;
        let next_root = roots[(bar + 1) % 4];

        // Melody (staccato celesta).
        for (step, &key) in melody[i].iter().enumerate() {
            if key != 0 {
                push(b + step, 1.4, MELODY, key, 100);
            }
        }

        // Bass: tiptoe root/fifth with a 2-note chromatic walk into the next root.
        push(b, 1.8, BASS, roots[i], 96);
        push(b + 4, 1.8, BASS, roots[i], 90);
        push(b + 8, 1.8, BASS, fifths[i], 90);
        push(b + 12, 1.6, BASS, next_root - 2, 82);
        push(b + 14, 1.6, BASS, next_root - 1, 88);

        // Vibraphone chord stabs on the "and" of beats 2 and 4 (syncopated).
        for &key in chords[i] {
            push(b + 6, 2.0, CHORDS, key, 72);
            push(b + 14, 2.0, CHORDS, key, 72);
        }

        // Percussion groove.
        for s in (0..16).step_by(2) {
            push(b + s, 1.0, DRUMS, 42, 44); // closed hi-hat (tiptoe)
        }
        for s in [1, 5, 9, 13] {
            push(b + s, 1.0, DRUMS, 82, 34); // shaker sparkle on the off-16ths
        }
        push(b, 1.0, DRUMS, 36, 110); // kick
        push(b + 10, 1.0, DRUMS, 36, 92); // syncopated kick
        push(b + 8, 1.0, DRUMS, 37, 84); // side stick on beat 3
        if i == 3 {
            // Little fill at the end of each 4-bar phrase.
            push(b + 12, 1.0, DRUMS, 38, 88);
            push(b + 14, 1.0, DRUMS, 38, 96);
            push(b + 15, 1.0, DRUMS, 38, 104);
        }
    }

    let end = 8 * 16;
    push(end, 8.0, MELODY, 76, 100); // E5
    push(end, 8.0, MELODY, 88, 80); // E6 sparkle
    for &key in chords[0] {
        push(end, 8.0, CHORDS, key, 78);
    }
    push(end, 8.0, BASS, 40, 92); // E2
    push(end, 1.0, DRUMS, 49, 110); // crash
    push(end, 1.0, DRUMS, 36, 110); // kick

    notes
}

fn write_wav(left: &[f32], right: &[f32], sample_rate: u32, path: &PathBuf) {
    // Normalize to avoid clipping.
    let max = left
        .iter()
        .chain(right.iter())
        .fold(0_f32, |m, v| m.max(v.abs()))
        .max(f32::EPSILON);
    let gain = 0.99_f32 / max;

    let num_frames = left.len();
    let channels: u16 = 2;
    let bits: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
    let block_align = channels * (bits / 8);
    let data_len = (num_frames * channels as usize * (bits / 8) as usize) as u32;

    let mut buf: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());

    for t in 0..num_frames {
        let l = (gain * left[t] * 32767_f32) as i16;
        let r = (gain * right[t] * 32767_f32) as i16;
        buf.extend_from_slice(&l.to_le_bytes());
        buf.extend_from_slice(&r.to_le_bytes());
    }

    let mut out = File::create(path).unwrap();
    out.write_all(&buf).unwrap();
}
