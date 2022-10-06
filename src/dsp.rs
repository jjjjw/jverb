use core::f32::consts::{SQRT_2, TAU};
use std::cmp::Ordering;

// Utility functions
pub fn get_max_float(values: &[f32]) -> f32 {
    let mut max = 0.0;
    for value in values.iter() {
        if value.total_cmp(&max) == Ordering::Greater {
            max = *value;
        }
    }

    max
}

// Uniform random between 0.3 and 0.8
pub const DELAYS: [f32; 32] = [
    0.7635944581685638,
    0.5054930849771431,
    0.6367330554100172,
    0.5988550749564814,
    0.5627433438493729,
    0.5292359969882741,
    0.39609197804744667,
    0.44730718318451523,
    0.6209089620338925,
    0.3002814785332507,
    0.691114361058875,
    0.5725195228704965,
    0.5018684398985229,
    0.40848200614524044,
    0.38804969100135855,
    0.547990456049264,
    0.5411968694743376,
    0.5355559000178921,
    0.7761144831398028,
    0.5059573559952342,
    0.418257481544177,
    0.4566727793084992,
    0.41703684491687243,
    0.7100284150808744,
    0.7510161891522189,
    0.4399213022865711,
    0.38414361694332666,
    0.6607107641090827,
    0.6706640816518474,
    0.47339770016067245,
    0.7967096565876597,
    0.6578104668408047,
];

pub const DEFAULT_SAMPLE_RATE: usize = 44100;

// Main DSP
pub struct Reverb {
    mix: f32,
    fdn: HouseholderFDN<{ DELAYS.len() }>,
    junction: ChannelJunction<2, { DELAYS.len() }>,
}

impl Reverb {
    pub fn new(mix: f32, lowpass: f32, time: f32, max_delay: usize) -> Self {
        let mut fdn = HouseholderFDN::<{ DELAYS.len() }>::new(
            DELAYS.map(|delay| (delay * DEFAULT_SAMPLE_RATE as f32) as usize),
            time,
            max_delay,
        );

        fdn.set_cutoff(lowpass);

        let junction = ChannelJunction::<2, { DELAYS.len() }>::default();

        Self { mix, fdn, junction }
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix;
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.fdn.set_gain(gain);
    }

    pub fn set_delays(&mut self, delays: [usize; DELAYS.len()]) {
        self.fdn.set_delays(delays);
    }

    pub fn set_max_delays(&mut self, max_delay: usize) -> () {
        self.fdn.set_max_delays(max_delay);
    }

    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.fdn.set_cutoff(cutoff);
    }

    pub fn reset(&mut self) {
        self.fdn.reset();
    }

    pub fn process_buffer_slice(&mut self, channels: &mut [&mut [f32]]) {
        // Simple equal power dry/wet mix
        let (wet_t, dry_t) = (self.mix.sqrt(), (1.0 - self.mix).sqrt());

        for ii in 0..channels[0].len() {
            let samples = [channels[0][ii], channels[1][ii]];

            let output = self
                .junction
                .join(self.fdn.tick(self.junction.split(samples)));

            channels[0][ii] = (channels[0][ii] * dry_t) + (output[0] * wet_t);
            channels[1][ii] = (channels[1][ii] * dry_t) + (output[1] * wet_t);
        }
    }
}

struct ChannelJunction<const INPUT: usize, const OUTPUT: usize> {
    input_buffer: [f32; INPUT],
    output_buffer: [f32; OUTPUT],
}

impl<const INPUT: usize, const OUTPUT: usize> Default for ChannelJunction<INPUT, OUTPUT> {
    fn default() -> Self {
        Self {
            input_buffer: [0.0; INPUT],
            output_buffer: [0.0; OUTPUT],
        }
    }
}

impl<const INPUT: usize, const OUTPUT: usize> ChannelJunction<INPUT, OUTPUT> {
    fn split(&self, input: [f32; INPUT]) -> [f32; OUTPUT] {
        let section_len = OUTPUT / INPUT;
        let mut curr_section_len = 0;
        let mut section_index = 0;

        self.output_buffer.map(|_ii| {
            let output = input[section_index];
            curr_section_len += 1;
            if curr_section_len >= section_len {
                curr_section_len = 0;
                section_index += 1;
            }
            output
        })
    }

    fn join(&self, output: [f32; OUTPUT]) -> [f32; INPUT] {
        let section_len = OUTPUT / INPUT;
        let mut section_index = 0;
        let avg = 1.0 / section_len as f32;

        self.input_buffer.map(|_ii| {
            let section_end = section_index + section_len;
            let average = output[section_index..section_end].iter().sum::<f32>() * avg;
            section_index = section_end;
            average
        })
    }
}

trait Signal {
    /// Process one sample
    fn tick(&mut self, input: f32) -> f32;

    fn reset(&mut self) -> ();
}

trait MultiSignal<const CHANNELS: usize> {
    /// Process one sample for multiple channels
    fn tick(&mut self, input: [f32; CHANNELS]) -> [f32; CHANNELS];

    fn reset(&mut self) -> ();
}

// Delay a signal a whole number of samples
struct IntegerDelay {
    buffer: Vec<f32>,
    delay: usize,
    write_index: usize,
}

impl IntegerDelay {
    fn new(max_delay: usize, delay: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay],
            delay: delay,
            write_index: 0,
        }
    }

    fn set_delay(&mut self, delay: usize) -> () {
        if delay == self.delay {
            return;
        }

        let old_delay = self.delay;

        // Delay can't be longer than the max delay length
        self.delay = delay.min(self.buffer.len() - 1);

        // Clear the buffer. It can be fun not to, however
        if self.delay < old_delay {
            for ii in self.delay..old_delay {
                self.buffer[ii] = 0.0;
            }
        }
    }

    fn set_max_delay(&mut self, max_delay: usize) -> () {
        self.buffer.resize(max_delay, 0.0);
    }
}

impl Signal for IntegerDelay {
    fn tick(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.write_index];
        self.buffer[self.write_index] = input;

        self.write_index += 1;
        if self.write_index >= self.delay {
            self.write_index = 0;
        }
        output
    }

    fn reset(&mut self) -> () {
        for sample in self.buffer.iter_mut() {
            *sample = 0.0;
        }
    }
}

struct Feedback<T: Signal> {
    signal: T,
    value: f32,
    gain: f32,
}

impl<T: Signal> Signal for Feedback<T> {
    fn tick(&mut self, input: f32) -> f32 {
        let fback = input + self.value;
        let output = self.signal.tick(fback) * self.gain;
        self.value = output;
        output
    }

    fn reset(&mut self) -> () {
        self.value = 0.0;
    }
}

impl<T: Signal> Feedback<T> {
    fn new(signal: T, gain: f32) -> Self {
        Self {
            signal: signal,
            gain: gain,
            value: 0.0,
        }
    }

    fn set_gain(&mut self, gain: f32) -> () {
        self.gain = gain;
    }
}

#[derive(Clone, Copy)]
struct OnePole {
    y1: f32,
    a0: f32,
    b1: f32,
}

// // A one pole filter, https://ccrma.stanford.edu/~jos/fp/One_Pole.html
impl Signal for OnePole {
    fn tick(&mut self, input: f32) -> f32 {
        self.y1 = input * self.a0 + self.y1 * self.b1;
        self.y1
    }

    fn reset(&mut self) -> () {
        self.y1 = 0.0;
    }
}

impl OnePole {
    fn new(cutoff: f32) -> Self {
        let mut filter = Self::default();
        filter.set_cutoff(cutoff);
        filter
    }

    fn set_cutoff(&mut self, cutoff: f32) -> () {
        let x = (-TAU * cutoff).exp();
        self.a0 = 1.0 - x;
        self.b1 = x;
    }
}

impl Default for OnePole {
    fn default() -> Self {
        Self {
            y1: 0.0,
            a0: 1f32,
            b1: 0.0,
        }
    }
}

struct HouseholderFDN<const SIZE: usize> {
    delays: [IntegerDelay; SIZE],
    filters: [OnePole; SIZE],
    values: [f32; SIZE],
    gain: f32,
}

impl<const SIZE: usize> HouseholderFDN<SIZE> {
    fn new(delays: [usize; SIZE], gain: f32, max_delay: usize) -> Self {
        let delays = delays.map(|delay| IntegerDelay::new(max_delay, delay));

        Self {
            delays: delays,
            filters: [OnePole::default(); SIZE],
            gain: gain,
            values: [0.0; SIZE],
        }
    }

    fn set_gain(&mut self, gain: f32) -> () {
        self.gain = gain;
    }

    fn set_delays(&mut self, delays: [usize; SIZE]) -> () {
        for (ii, delay) in delays.iter().enumerate() {
            self.delays[ii].set_delay(*delay);
        }
    }

    fn set_max_delays(&mut self, max_delay: usize) -> () {
        for delay in self.delays.iter_mut() {
            delay.set_max_delay(max_delay);
        }
    }

    fn set_cutoff(&mut self, cutoff: f32) -> () {
        for filter in self.filters.iter_mut() {
            filter.set_cutoff(cutoff);
        }
    }
}

impl<const CHANNELS: usize> MultiSignal<CHANNELS> for HouseholderFDN<CHANNELS> {
    fn tick(&mut self, input: [f32; CHANNELS]) -> [f32; CHANNELS] {
        let mut output = input;

        // Run the delay lines
        for (ii, sample) in output.iter_mut().enumerate() {
            let input = *sample + self.values[ii];
            *sample = self.filters[ii].tick(self.delays[ii].tick(input)) * self.gain;
        }

        // Householder feedback matrix. All outputs are summed and fed back into all inputs
        // https://github.com/madronalabs/madronalib/blob/master/source/DSP/MLDSPFilters.h#L953
        // https://ccrma.stanford.edu/~jos/pasp/Householder_Feedback_Matrix.html
        let mut delay_sum: f32 = output.iter().sum();
        delay_sum *= 2.0 / CHANNELS as f32;

        // Set the feedback, all delays are fed back into each other
        for (ii, value) in self.values.iter_mut().enumerate() {
            *value = output[ii] - delay_sum;
        }

        output
    }

    fn reset(&mut self) -> () {
        for filter in self.filters.iter_mut() {
            filter.reset();
        }
        for delay in self.delays.iter_mut() {
            delay.reset();
        }
        for value in self.values.iter_mut() {
            *value = 0.0;
        }
    }
}

struct HadamardFDN<const SIZE: usize> {
    delays: [IntegerDelay; SIZE],
    filters: [OnePole; SIZE],
    values: [f32; SIZE],
    gain: f32,
}

impl<const SIZE: usize> HadamardFDN<SIZE> {
    fn new(delays: [usize; SIZE], gain: f32, max_delay: usize) -> Self {
        let delays = delays.map(|delay| IntegerDelay::new(max_delay, delay));

        Self {
            delays: delays,
            filters: [OnePole::default(); SIZE],
            gain: gain,
            values: [0.0; SIZE],
        }
    }

    fn set_gain(&mut self, gain: f32) -> () {
        self.gain = gain;
    }

    fn set_delays(&mut self, delays: [usize; SIZE]) -> () {
        for (ii, delay) in delays.iter().enumerate() {
            self.delays[ii].set_delay(*delay);
        }
    }

    fn set_max_delays(&mut self, max_delay: usize) -> () {
        for delay in self.delays.iter_mut() {
            delay.set_max_delay(max_delay);
        }
    }

    fn set_cutoff(&mut self, cutoff: f32) -> () {
        for filter in self.filters.iter_mut() {
            filter.set_cutoff(cutoff);
        }
    }
}

impl<const CHANNELS: usize> MultiSignal<CHANNELS> for HadamardFDN<CHANNELS> {
    fn tick(&mut self, input: [f32; CHANNELS]) -> [f32; CHANNELS] {
        let mut output = input;

        // Run the delay lines
        for (ii, sample) in output.iter_mut().enumerate() {
            let input = *sample + self.values[ii];
            *sample = self.filters[ii].tick(self.delays[ii].tick(input)) * self.gain;
        }

        // Hadamard feedback matrix
        // https://ccrma.stanford.edu/~jos/pasp/Hadamard_Matrix.html
        // https://github.com/SamiPerttu/fundsp/blob/50811676691a3d066964241e344987d4c45c3e9d/src/feedback.rs#L9
        let mut h = 1;
        while h < CHANNELS {
            let mut i = 0;
            while i < CHANNELS {
                for j in i..i + h {
                    let x = output[j];
                    let y = output[j + h];
                    output[j] = x + y;
                    output[j + h] = x - y;
                }
                i += h * 2;
            }
            h *= 2;
        }

        // Normalization for up to 511 channels.
        let mut c = 1.0;
        if CHANNELS >= 256 {
            c = 1.0 / 16.0;
        } else if CHANNELS >= 128 {
            c = 1.0 / (SQRT_2 * 8.0);
        } else if CHANNELS >= 64 {
            c = 1.0 / 8.0;
        } else if CHANNELS >= 32 {
            c = 1.0 / (SQRT_2 * 4.0);
        } else if CHANNELS >= 16 {
            c = 1.0 / 4.0;
        } else if CHANNELS >= 8 {
            c = 1.0 / (SQRT_2 * 2.0);
        } else if CHANNELS >= 4 {
            c = 1.0 / 2.0;
        } else if CHANNELS >= 2 {
            c = 1.0 / SQRT_2;
        }

        output = output.map(|x| x * c);

        // Set the feedback
        for (ii, value) in self.values.iter_mut().enumerate() {
            *value = output[ii];
        }

        output
    }

    fn reset(&mut self) -> () {
        for filter in self.filters.iter_mut() {
            filter.reset();
        }
        for delay in self.delays.iter_mut() {
            delay.reset();
        }
        for value in self.values.iter_mut() {
            *value = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_no_alloc::*;

    #[global_allocator]
    static A: AllocDisabler = AllocDisabler;

    #[test]
    fn test_delay() {
        let mut delay = IntegerDelay::new(10, 10);

        assert_eq!(delay.tick(1.0), 0.0);

        for _i in 0..10 {
            delay.tick(1.0);
        }

        assert_eq!(delay.tick(1.0), 1.0);

        for _i in 0..10 {
            delay.tick(0.0);
        }

        assert_eq!(delay.tick(1.0), 0.0);
    }

    #[test]
    fn test_delay_entire_buffer() {
        let mut delay = IntegerDelay::new(10, 1);

        for i in 0..10 {
            delay.tick(i as f32);
        }

        assert_eq!(delay.tick(1.0), 9.0);
        assert_eq!(delay.tick(1.0), 1.0);
    }

    #[test]
    fn test_change_delay() {
        let mut delay = IntegerDelay::new(10, 1);

        for i in 0..10 {
            delay.tick(i as f32);
        }

        assert_eq!(delay.tick(1.0), 9.0);

        delay.set_delay(2);
        assert_eq!(delay.tick(0.5), 1.0);
        assert_eq!(delay.tick(0.25), 0.0);
        assert_eq!(delay.tick(0.1), 0.5);
        assert_eq!(delay.tick(0.01), 0.25);
    }

    #[test]
    fn test_one_pole_lowpass() {
        let mut lowpass = OnePole::new(0.09);

        assert_eq!(lowpass.tick(1.0), 0.43191642);
        assert_eq!(lowpass.tick(1.0), 0.677281);

        lowpass.set_cutoff(1.0);
        assert_eq!(lowpass.tick(1.0), 0.9993974);
    }

    #[test]
    fn test_feedback() {
        let delay = IntegerDelay::new(10, 1);

        let mut feedback = Feedback::<IntegerDelay>::new(delay, 0.5);

        assert_eq!(feedback.tick(1.0), 0.0);
        assert_eq!(feedback.tick(1.0), 0.5);
        assert_eq!(feedback.tick(1.0), 0.5);
        assert_eq!(feedback.tick(1.0), 0.75);
        assert_eq!(feedback.tick(1.0), 0.75);
        assert_eq!(feedback.tick(1.0), 0.875);
    }

    #[test]
    fn test_feedback_change_gain() {
        let delay = IntegerDelay::new(10, 1);

        let mut feedback = Feedback::<IntegerDelay>::new(delay, 0.5);

        assert_eq!(feedback.tick(1.0), 0.0);
        assert_eq!(feedback.tick(1.0), 0.5);

        feedback.set_gain(1.0);

        assert_eq!(feedback.tick(1.0), 1.0);
        assert_eq!(feedback.tick(1.0), 1.5);
        assert_eq!(feedback.tick(1.0), 2.0);
    }

    #[test]
    fn test_junction() {
        let junction = ChannelJunction::<2, 32>::default();

        assert_eq!(junction.split([1.0, 1.0]), [1.0; 32]);

        let mut input = [0.25; 32];
        for ii in 0..16 {
            input[ii] = 1.0;
        }

        assert_eq!(junction.split([1.0, 0.25]), input);

        assert_eq!(junction.join([1.0; 32]), [1.0, 1.0]);

        let mut output = [0.25; 32];
        for ii in 0..16 {
            output[ii] = 1.0;
        }

        assert_eq!(junction.join(output), [1.0, 0.25]);
    }

    #[test]
    fn test_householder_fdn() {
        const DELAYS: [usize; 4] = [2, 3, 5, 7];
        const DELAYS_LEN: usize = DELAYS.len();

        let mut fdn = HouseholderFDN::<{ DELAYS_LEN }>::new(DELAYS, 0.5, 10);

        let junction = ChannelJunction::<2, { DELAYS_LEN }>::default();

        for _i in 0..10 {
            fdn.tick(junction.split([1.0, 1.0]));
        }

        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.296875, 0.3125]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.25390625, 0.296875]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.31640625, 0.328125]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.30859375, 0.171875]
        );
    }

    #[test]
    fn test_householder_fdn_lowpass() {
        const DELAYS: [usize; 4] = [2, 3, 5, 7];
        const DELAYS_LEN: usize = DELAYS.len();

        let mut fdn = HouseholderFDN::<{ DELAYS_LEN }>::new(DELAYS, 1.0, 10);

        fdn.set_cutoff(0.09);

        let junction = ChannelJunction::<2, { DELAYS_LEN }>::default();

        for _i in 0..10 {
            fdn.tick(junction.split([1.0, 1.0]));
        }

        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.70215225, 0.64007735]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.52303684, 0.52741337]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.41039184, 0.44365278]
        );
    }

    #[test]
    fn test_hadamard_algo() {
        let mut example_output = [1.0; 4];

        // Hardcoded hadamard matrix
        let example_matrix: [[i32; 4]; 4] =
            [[1, 1, 1, 1], [1, -1, 1, -1], [1, 1, -1, -1], [1, -1, -1, 1]];

        for (ii, row) in example_matrix.iter().enumerate() {
            let mut sum = 0;
            for (ii, val) in row.iter().enumerate() {
                sum += 1 * *val;
            }
            example_output[ii] = sum as f32;
        }

        let mut algo_output = [1.0; 4];

        // The algo used in the Hadamard FDN implementation
        let mut h = 1;
        while h < 4 {
            let mut i = 0;
            while i < 4 {
                for j in i..i + h {
                    let x = algo_output[j];
                    let y = algo_output[j + h];
                    algo_output[j] = x + y;
                    algo_output[j + h] = x - y;
                }
                i += h * 2;
            }
            h *= 2;
        }

        assert_eq!(algo_output, example_output);
    }

    #[test]
    fn test_hadamard_fdn() {
        const DELAYS: [usize; 4] = [2, 3, 5, 7];
        const DELAYS_LEN: usize = DELAYS.len();

        let mut fdn = HadamardFDN::new(DELAYS, 0.5, 10);

        let junction = ChannelJunction::<2, { DELAYS_LEN }>::default();

        for _i in 0..10 {
            fdn.tick(junction.split([1.0, 1.0]));
        }

        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.90625, 0.15625]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.89453125, 0.23828125]
        );
        assert_eq!(
            junction.join(fdn.tick(junction.split([1.0, 1.0]))),
            [0.96875, 0.25]
        );
    }

    #[test]
    fn test_sort() {
        assert_eq!(get_max_float(&[0.1, 0.2, 0.3]), 0.3);
    }

    #[test]
    fn test_reverb_no_alloc() {
        let mut reverb = Reverb::new(
            0.5,
            0.9,
            0.9,
            (DEFAULT_SAMPLE_RATE as f32 * get_max_float(&DELAYS)) as usize,
        );

        assert_no_alloc(|| {
            reverb.set_mix(0.75);
            reverb.set_gain(2.0);
            reverb.set_delays(
                DELAYS.map(|delay| (delay * 0.5 * DEFAULT_SAMPLE_RATE as f32) as usize),
            );
            reverb.set_cutoff(1.0);

            reverb.process_buffer_slice(&mut [&mut [0.5; 64], &mut [0.5; 64]]);
        });
    }
}
