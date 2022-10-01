mod dsp;

use dsp::*;
use nih_plug::prelude::*;
use std::sync::Arc;

const DEFAULT_SAMPLE_RATE: usize = 44100;
// Stolen from: https://github.com/SamiPerttu/fundsp/blob/50811676691a3d066964241e344987d4c45c3e9d/src/prelude.rs#L1469
const DELAYS: [f32; 32] = [
    0.073904, 0.052918, 0.066238, 0.066387, 0.037783, 0.080073, 0.050961, 0.075900, 0.043646,
    0.072095, 0.056194, 0.045961, 0.058934, 0.068016, 0.047529, 0.058156, 0.072972, 0.036084,
    0.062715, 0.076377, 0.044339, 0.076725, 0.077884, 0.046126, 0.067741, 0.049800, 0.051709,
    0.082923, 0.070121, 0.079315, 0.055039, 0.081859,
];
const MAX_SIZE: f32 = 10.0;

struct Jverb {
    params: Arc<JverbParams>,
    audio: HouseholderFDN,
    delays_vec: Vec<f32>,
}

#[derive(Params)]
struct JverbParams {
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "size"]
    pub size: FloatParam,
    #[id = "time"]
    pub time: FloatParam,
    #[id = "lowpass"]
    pub lowpass: FloatParam,
}

impl Default for Jverb {
    fn default() -> Self {
        let default_params = JverbParams::default();
        let time = default_params.time.smoothed.next();
        let lowpass = default_params.lowpass.smoothed.next();

        let delays_vec = DELAYS.to_vec();

        let mut fdn = HouseholderFDN::new(
            // Simple testing primes
            // vec![
            //     (DEFAULT_SAMPLE_RATE as f32 * 0.02) as usize,
            //     (DEFAULT_SAMPLE_RATE as f32 * 0.03) as usize,
            //     (DEFAULT_SAMPLE_RATE as f32 * 0.05) as usize,
            //     (DEFAULT_SAMPLE_RATE as f32 * 0.07) as usize
            // ],
            delays_vec
                .iter()
                .map(|delay| (delay * DEFAULT_SAMPLE_RATE as f32) as usize)
                .collect(),
            time,
            (MAX_SIZE * DEFAULT_SAMPLE_RATE as f32 * get_max_float(&delays_vec)) as usize, // Max buffer size
        );

        fdn.set_cutoff(lowpass);

        Self {
            params: Arc::new(default_params),
            delays_vec,
            audio: fdn,
        }
    }
}

impl Default for JverbParams {
    fn default() -> Self {
        Self {
            // Dry/Wet percent
            mix: FloatParam::new("Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(1.0))
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            // Reverb size
            size: FloatParam::new(
                "Size",
                1.0,
                FloatRange::Linear {
                    min: 0.5,
                    max: MAX_SIZE,
                },
            )
            .with_smoother(SmoothingStyle::Linear(1.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            // Reverb time
            time: FloatParam::new("Time", 0.9, FloatRange::Linear { min: 0.8, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(1.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            // Lowpass cutoff
            lowpass: FloatParam::new(
                "Lowpass",
                0.25,
                FloatRange::Linear {
                    min: 0.001,
                    max: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::Linear(1.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl Plugin for Jverb {
    const NAME: &'static str = "jverb";
    const VENDOR: &'static str = "JJ";
    const URL: &'static str = "https://www.example.com";
    const EMAIL: &'static str = "jj.weber@gmail.com";

    const VERSION: &'static str = "0.0.1";

    const DEFAULT_INPUT_CHANNELS: u32 = 2;
    const DEFAULT_OUTPUT_CHANNELS: u32 = 2;

    const DEFAULT_AUX_INPUTS: Option<AuxiliaryIOConfig> = None;
    const DEFAULT_AUX_OUTPUTS: Option<AuxiliaryIOConfig> = None;

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn accepts_bus_config(&self, config: &BusConfig) -> bool {
        // Works with any symmetrical channel input
        config.num_input_channels == config.num_output_channels && config.num_input_channels > 0
    }

    fn initialize(
        &mut self,
        _bus_config: &BusConfig,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.

        // TODO: what if this changes?
        let sample_rate = buffer_config.sample_rate;

        self.audio
            .set_max_delays((MAX_SIZE * sample_rate * get_max_float(&self.delays_vec)) as usize);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext,
    ) -> ProcessStatus {
        let mix = self.params.mix.smoothed.next();
        let size = self.params.size.smoothed.next();
        let time = self.params.time.smoothed.next();
        let lowpass = self.params.lowpass.smoothed.next();

        let sample_rate = context.transport().sample_rate;

        self.audio.set_gain(time);
        self.audio.set_delays(
            self.delays_vec
                .iter()
                .map(|delay| (delay * size * sample_rate as f32) as usize)
                .collect(),
        );
        self.audio.set_cutoff(lowpass);

        // Simple equal power dry/wet mix
        let (wet_t, dry_t) = (mix.sqrt(), (1.0 - mix).sqrt());

        let channels = buffer.as_slice();

        for ii in 0..channels[0].len() {
            let samples = (0..channels.len()).map(|yy| channels[yy][ii]).collect();

            let output = self.audio.process(samples);

            for yy in 0..channels.len() {
                channels[yy][ii] = (channels[yy][ii] * dry_t) + (output[yy] * wet_t);
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Jverb {
    const CLAP_ID: &'static str = "com.your-domain.jverb";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Reverb");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for Jverb {
    const VST3_CLASS_ID: [u8; 16] = *b"jverb00000000000";
    const VST3_CATEGORIES: &'static str = "Fx";
}

nih_export_clap!(Jverb);
nih_export_vst3!(Jverb);
