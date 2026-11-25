//! Sound effects and music, mixed by [kira](https://docs.rs/kira).

use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kira::backend::Backend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::track::MainTrackBuilder;
use kira::{
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Panning, PlaybackRate, Tween,
};

use crate::Error;

/// A sound loaded into memory, ready to play any number of times at once.
///
/// Cloning is cheap: clones share the same samples.
#[derive(Clone)]
pub struct Sound {
    data: StaticSoundData,
}

impl Sound {
    /// A sound from the bytes of a WAV or Ogg Vorbis file. The format is
    /// detected from the data.
    ///
    /// Embed sounds with `include_bytes!` so the game runs from any working
    /// directory (and in browsers, which have no file system).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let data = StaticSoundData::from_cursor(Cursor::new(bytes.to_vec()))
            .map_err(|e| Error::Audio(format!("could not decode sound: {e}")))?;
        Ok(Self { data })
    }

    /// A sound from a WAV or Ogg Vorbis file on disk.
    ///
    /// Browsers have no file system, so this always fails there; embed
    /// sounds with `include_bytes!` and use [`Sound::from_bytes`] instead.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Audio(format!("could not read {}: {e}", path.display())))?;
        Self::from_bytes(&bytes).map_err(|e| Error::Audio(format!("{}: {e}", path.display())))
    }

    /// A mono sound from raw samples, each from -1.0 to 1.0, played at
    /// `sample_rate` samples per second. Use this for sounds made in code.
    pub fn from_samples(sample_rate: u32, samples: &[f32]) -> Result<Self, Error> {
        if sample_rate == 0 {
            return Err(Error::Audio("sample rate must be above zero".into()));
        }
        let frames: Arc<[kira::Frame]> =
            samples.iter().map(|&s| kira::Frame::from_mono(s)).collect();
        Ok(Self {
            data: StaticSoundData {
                sample_rate,
                frames,
                settings: StaticSoundSettings::default(),
                slice: None,
            },
        })
    }

    /// How long the sound plays at normal speed.
    pub fn duration(&self) -> Duration {
        self.data.duration()
    }

    fn is_empty(&self) -> bool {
        self.data.num_frames() == 0
    }

    /// The samples with `params` applied, ready to hand to kira.
    fn with_params(&self, params: SoundParams) -> StaticSoundData {
        self.data
            .volume(decibels(params.volume))
            .playback_rate(PlaybackRate(params.speed.max(0.0) as f64))
            .panning(Panning(params.pan.clamp(-1.0, 1.0)))
    }
}

impl fmt::Debug for Sound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sound")
            .field("duration", &self.duration())
            .field("sample_rate", &self.data.sample_rate)
            .finish()
    }
}

/// Options for [`crate::Context::play_sound_with`] and
/// [`crate::Context::play_music_with`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundParams {
    /// Loudness: 1.0 plays the sound as recorded, 0.5 at half the amplitude,
    /// 0.0 is silent.
    pub volume: f32,
    /// Playback speed: 1.0 is normal; 2.0 plays twice as fast and an octave
    /// higher. Small random changes make repeated sounds less monotonous.
    pub speed: f32,
    /// Stereo position: -1.0 is fully left, 0.0 centered, 1.0 fully right.
    pub pan: f32,
}

impl Default for SoundParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            speed: 1.0,
            pan: 0.0,
        }
    }
}

/// Converts a volume (amplitude, 1.0 = unchanged) to the decibels kira uses.
fn decibels(volume: f32) -> Decibels {
    if volume > 0.0 {
        Decibels((20.0 * volume.log10()).max(Decibels::SILENCE.0))
    } else {
        Decibels::SILENCE
    }
}

/// How long music fades out when stopped or replaced, so it doesn't click.
const MUSIC_FADE: Duration = Duration::from_millis(100);

/// The engine's audio output. Generic over the kira backend so tests can
/// capture the mixed samples instead of using a sound card.
pub(crate) struct Audio<B: Backend = DefaultBackend> {
    output: Output<B>,
    /// Master volume, as set by the game.
    volume: f32,
    /// The music the game asked for, kept even before the output starts so
    /// it can begin then.
    music: Option<Music>,
}

enum Output<B: Backend> {
    /// Not opened yet. Browsers allow sound only after the player clicks or
    /// presses a key, so on the web the engine waits for that.
    Waiting,
    /// Boxed: the manager is large, and the other states hold nothing.
    Running(Box<AudioManager<B>>),
    /// No sound device, or it failed to open. The game runs silently.
    Off,
}

struct Music {
    sound: Sound,
    params: SoundParams,
    /// Set while it plays.
    handle: Option<StaticSoundHandle>,
}

impl<B: Backend> Audio<B>
where
    B::Settings: Default,
    B::Error: fmt::Display,
{
    pub(crate) fn new() -> Self {
        Self {
            output: Output::Waiting,
            volume: 1.0,
            music: None,
        }
    }

    /// Opens the sound device. Does nothing if it was already tried.
    pub(crate) fn start(&mut self) {
        if !matches!(self.output, Output::Waiting) {
            return;
        }
        let settings = AudioManagerSettings {
            main_track_builder: MainTrackBuilder::new().volume(decibels(self.volume)),
            ..AudioManagerSettings::default()
        };
        match AudioManager::<B>::new(settings) {
            Ok(manager) => {
                log::info!("sound on");
                self.output = Output::Running(Box::new(manager));
                if let Some(music) = self.music.take() {
                    self.play_music(&music.sound, music.params);
                }
            }
            Err(e) => {
                log::warn!("sound is off: could not open the audio device: {e}");
                self.output = Output::Off;
            }
        }
    }

    pub(crate) fn play(&mut self, sound: &Sound, params: SoundParams) {
        // Before the output starts, sound effects are dropped rather than
        // saved up: they would all play at once, late.
        if let Output::Running(manager) = &mut self.output
            && !sound.is_empty()
            && let Err(e) = manager.play(sound.with_params(params))
        {
            log::debug!("sound not played: {e}");
        }
    }

    pub(crate) fn play_music(&mut self, sound: &Sound, params: SoundParams) {
        self.stop_music();
        let handle = match &mut self.output {
            // kira never finishes looping a sound with no samples, so an
            // empty sound is never handed to it.
            Output::Running(_) if sound.is_empty() => None,
            Output::Running(manager) => {
                let data = sound.with_params(params).loop_region(..);
                manager
                    .play(data)
                    .inspect_err(|e| log::warn!("music not played: {e}"))
                    .ok()
            }
            Output::Waiting | Output::Off => None,
        };
        self.music = Some(Music {
            sound: sound.clone(),
            params,
            handle,
        });
    }

    pub(crate) fn stop_music(&mut self) {
        if let Some(Music {
            handle: Some(mut handle),
            ..
        }) = self.music.take()
        {
            handle.stop(Tween {
                duration: MUSIC_FADE,
                ..Tween::default()
            });
        }
    }

    pub(crate) fn volume(&self) -> f32 {
        self.volume
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0);
        if let Output::Running(manager) = &mut self.output {
            manager
                .main_track()
                .set_volume(decibels(self.volume), Tween::default());
        }
    }

    #[cfg(test)]
    fn manager(&mut self) -> &mut AudioManager<B> {
        match &mut self.output {
            Output::Running(manager) => manager,
            _ => panic!("audio is not running"),
        }
    }
}

impl<B: Backend> fmt::Debug for Audio<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self.output {
            Output::Waiting => "waiting",
            Output::Running(_) => "running",
            Output::Off => "off",
        };
        f.debug_struct("Audio")
            .field("output", &output)
            .field("volume", &self.volume)
            .field("music", &self.music.as_ref().map(|m| &m.sound))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use kira::backend::Renderer;

    use super::*;

    /// Low, so a second of sound is only 1000 samples.
    const RATE: u32 = 1000;

    /// A backend that mixes on demand into a buffer the test can read.
    struct Capture {
        renderer: Option<Renderer>,
    }

    impl Backend for Capture {
        type Settings = ();
        type Error = &'static str;

        fn setup(_: (), _internal_buffer_size: usize) -> Result<(Self, u32), Self::Error> {
            Ok((Self { renderer: None }, RATE))
        }

        fn start(&mut self, renderer: Renderer) -> Result<(), Self::Error> {
            self.renderer = Some(renderer);
            Ok(())
        }
    }

    /// Mixes the next `seconds` of output; returns (left, right) per sample.
    fn render(audio: &mut Audio<Capture>, seconds: f32) -> Vec<(f32, f32)> {
        let renderer = audio.manager().backend_mut().renderer.as_mut().unwrap();
        renderer.on_start_processing();
        let mut out = vec![0.0; (seconds * RATE as f32) as usize * 2];
        renderer.process(&mut out, 2);
        out.as_chunks().0.iter().map(|&[l, r]| (l, r)).collect()
    }

    /// The last sample of the next `seconds` of output.
    fn level_after(audio: &mut Audio<Capture>, seconds: f32) -> (f32, f32) {
        *render(audio, seconds).last().unwrap()
    }

    fn started() -> Audio<Capture> {
        let mut audio = Audio::new();
        audio.start();
        audio
    }

    /// A flat tone, so the output level shows which sounds are playing.
    fn constant(value: f32, seconds: f32) -> Sound {
        Sound::from_samples(RATE, &vec![value; (seconds * RATE as f32) as usize]).unwrap()
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
    }

    /// A minimal 16-bit PCM WAV file.
    fn wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = samples.len() as u32 * 2;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    #[test]
    fn decodes_wav() {
        let samples = [0, 8192, -8192, 16384].repeat(500); // 1000 stereo frames
        let sound = Sound::from_bytes(&wav(4000, 2, &samples)).unwrap();
        assert_eq!(sound.duration(), Duration::from_millis(250));
        let frame = sound.data.frame_at_index(1).unwrap();
        assert_near(frame.left, -0.25);
        assert_near(frame.right, 0.5);
    }

    #[test]
    fn decodes_ogg_vorbis() {
        // 0.5 s of a 440 Hz tone at 22050 Hz, mono.
        let sound = Sound::from_bytes(include_bytes!("../tests/data/tone.ogg")).unwrap();
        let ms = sound.duration().as_millis();
        assert!((490..=510).contains(&ms), "{ms} ms");
        let peak = (0..sound.data.num_frames())
            .map(|i| sound.data.frame_at_index(i).unwrap().left.abs())
            .fold(0.0, f32::max);
        assert!(peak > 0.3, "peak {peak}");
    }

    #[test]
    fn bad_sounds_are_errors() {
        assert!(Sound::from_bytes(b"not a sound").is_err());
        assert!(Sound::from_file("does/not/exist.wav").is_err());
        assert!(Sound::from_samples(0, &[0.0]).is_err());
    }

    #[test]
    fn volume_converts_to_decibels() {
        assert_eq!(decibels(1.0), Decibels::IDENTITY);
        assert_eq!(decibels(0.0), Decibels::SILENCE);
        assert_eq!(decibels(-1.0), Decibels::SILENCE);
        assert_eq!(decibels(f32::NAN), Decibels::SILENCE);
        assert_eq!(decibels(1e-9), Decibels::SILENCE);
        assert_near(decibels(0.5).as_amplitude(), 0.5);
    }

    #[test]
    fn sounds_play_once_and_mix() {
        let mut audio = started();
        audio.play(&constant(0.25, 0.2), SoundParams::default());
        audio.play(&constant(0.25, 0.4), SoundParams::default());
        assert_eq!(level_after(&mut audio, 0.1), (0.5, 0.5));
        // The first one has ended, the second is still playing.
        assert_eq!(level_after(&mut audio, 0.2), (0.25, 0.25));
        assert_eq!(level_after(&mut audio, 0.2), (0.0, 0.0));
    }

    #[test]
    fn params_change_volume_and_pan() {
        let mut audio = started();
        let half = SoundParams {
            volume: 0.5,
            ..SoundParams::default()
        };
        audio.play(&constant(0.4, 0.1), half);
        let (left, right) = level_after(&mut audio, 0.05);
        assert_near(left, 0.2);
        assert_near(right, 0.2);

        let mut audio = started();
        let left_only = SoundParams {
            pan: -1.0,
            ..SoundParams::default()
        };
        audio.play(&constant(0.4, 0.1), left_only);
        let (left, right) = level_after(&mut audio, 0.05);
        assert!(left > 0.4, "{left}"); // equal-power panning: louder on its side
        assert_near(right, 0.0);
    }

    #[test]
    fn speed_changes_length() {
        let mut audio = started();
        let double = SoundParams {
            speed: 2.0,
            ..SoundParams::default()
        };
        audio.play(&constant(0.5, 0.2), double);
        let out = render(&mut audio, 0.2);
        let playing = out.iter().filter(|(l, _)| *l > 0.25).count();
        assert!((95..=105).contains(&playing), "{playing} samples");
    }

    #[test]
    fn master_volume_scales_everything() {
        let mut audio = started();
        audio.set_volume(0.5);
        assert_eq!(audio.volume(), 0.5);
        audio.play(&constant(0.4, 1.0), SoundParams::default());
        assert_near(level_after(&mut audio, 0.1).0, 0.2);
        audio.set_volume(0.0);
        assert_eq!(level_after(&mut audio, 0.1), (0.0, 0.0));
        audio.set_volume(-3.0);
        assert_eq!(audio.volume(), 0.0);
    }

    #[test]
    fn music_loops_and_is_replaced() {
        let mut audio = started();
        audio.play_music(&constant(0.25, 0.1), SoundParams::default());
        // Still playing long after its 0.1 s length.
        assert_eq!(level_after(&mut audio, 0.55), (0.25, 0.25));

        // New music replaces the old one after a short fade.
        audio.play_music(&constant(0.5, 0.1), SoundParams::default());
        assert_eq!(level_after(&mut audio, 0.5), (0.5, 0.5));

        audio.stop_music();
        assert_eq!(level_after(&mut audio, 0.5), (0.0, 0.0));
    }

    #[test]
    fn before_start_music_waits_and_sounds_are_dropped() {
        let mut audio = Audio::<Capture>::new();
        audio.set_volume(0.5);
        audio.play(&constant(0.4, 1.0), SoundParams::default());
        audio.play_music(&constant(0.2, 0.1), SoundParams::default());
        audio.start();
        // Only the music plays, at the volume set before starting.
        assert_near(level_after(&mut audio, 0.3).0, 0.1);
    }

    #[test]
    fn music_stopped_before_start_never_plays() {
        let mut audio = Audio::<Capture>::new();
        audio.play_music(&constant(0.2, 0.1), SoundParams::default());
        audio.stop_music();
        audio.start();
        assert_eq!(level_after(&mut audio, 0.2), (0.0, 0.0));
    }

    #[test]
    fn empty_sound_plays_without_trouble() {
        let mut audio = started();
        let empty = Sound::from_samples(RATE, &[]).unwrap();
        assert_eq!(empty.duration(), Duration::ZERO);
        audio.play(&empty, SoundParams::default());
        audio.play_music(&empty, SoundParams::default());
        assert_eq!(level_after(&mut audio, 0.1), (0.0, 0.0));
    }

    #[test]
    fn without_a_device_the_game_runs_silently() {
        struct NoDevice;
        impl Backend for NoDevice {
            type Settings = ();
            type Error = &'static str;
            fn setup(_: (), _: usize) -> Result<(Self, u32), Self::Error> {
                Err("no device")
            }
            fn start(&mut self, _: Renderer) -> Result<(), Self::Error> {
                unreachable!()
            }
        }
        let mut audio = Audio::<NoDevice>::new();
        audio.start();
        audio.play(&constant(0.1, 0.1), SoundParams::default());
        audio.play_music(&constant(0.1, 0.1), SoundParams::default());
        audio.set_volume(0.3);
        audio.stop_music();
        assert!(format!("{audio:?}").contains("off"));
    }
}
