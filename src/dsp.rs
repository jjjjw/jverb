use core::f32::consts::TAU;

pub trait Signal {
    /// Process one sample
    fn tick(&mut self, input: f32) -> f32;
}

// Delay a signal a whole number of samples
pub struct IntegerDelay {
    buffer: Vec<f32>,
    delay: usize,
    write_index: usize,
}

impl IntegerDelay {
    pub fn new(max_delay: usize, delay: usize) -> Self {
        Self {
            buffer: vec![0f32; max_delay],
            delay: delay,
            write_index: 0,
        }
    }

    pub fn set_delay(&mut self, delay: usize) -> () {
        if delay == self.delay {
            return;
        }

        // Delay can't be longer than the max delay length
        self.delay = delay.min(self.buffer.len() - 1);

        // Clear the buffer. It can be fun not to, however
        for (ii, sample) in self.buffer.iter_mut().enumerate() {
            if ii >= self.delay {
                *sample = 0f32;
            }
        }
    }

    pub fn set_max_delay(&mut self, max_delay: usize) -> () {
        self.buffer.resize(max_delay, 0f32);
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
}

pub struct Feedback<T: Signal> {
    pub signal: T,
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
}

impl<T: Signal> Feedback<T> {
    pub fn new(signal: T, gain: f32) -> Self {
        Self {
            signal: signal,
            gain: gain,
            value: 0.0,
        }
    }

    pub fn set_gain(&mut self, gain: f32) -> () {
        self.gain = gain;
    }
}

#[derive(Clone)]
pub struct OnePole {
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
}

impl OnePole {
    pub fn new(cutoff: f32) -> Self {
        let mut filter = Self::default();
        filter.set_cutoff(cutoff);
        filter
    }

    pub fn set_cutoff(&mut self, cutoff: f32) -> () {
        let x = (-TAU * cutoff).exp();
        self.a0 = 1.0 - x;
        self.b1 = x;
    }
}

impl Default for OnePole {
    fn default() -> Self {
        Self {
            y1: 0f32,
            a0: 1f32,
            b1: 0f32,
        }
    }
}

pub struct HouseholderFDN {
    delays: Vec<IntegerDelay>,
    filters: Vec<OnePole>,
    values: Vec<f32>,
    gain: f32,
}

impl HouseholderFDN {
    pub fn new(delays: Vec<usize>, gain: f32, max_delay: usize) -> Self {
        let matrix_size = delays.len();
        let delays: Vec<IntegerDelay> = delays
            .iter()
            .map(|delay| IntegerDelay::new(max_delay, *delay))
            .collect();

        let filters = vec![OnePole::default(); matrix_size];

        Self {
            delays: delays,
            filters: filters,
            gain: gain,
            values: vec![0f32; matrix_size],
        }
    }

    fn split(input: Vec<f32>, target_len: usize) -> Vec<f32> {
        let input_len = input.len();
        let section_len = (target_len / input_len) as usize;
        let mut curr_section_len = 0;
        let mut section_index = 0;

        (0..target_len)
            .map(|_ii| {
                let output = input[section_index];
                curr_section_len += 1;
                if curr_section_len >= section_len {
                    curr_section_len = 0;
                    section_index += 1;
                }
                output
            })
            .collect()
    }

    fn join(input: Vec<f32>, target_len: usize) -> Vec<f32> {
        let input_len = input.len();
        let section_len = (input_len / target_len) as usize;
        let mut section_index = 0;
        let avg = 1.0 / section_len as f32;

        (0..target_len)
            .map(|_ii| {
                let section_end = section_index + section_len;
                let average = input[section_index..section_end].iter().sum::<f32>() * avg;
                section_index = section_end;
                average
            })
            .collect()
    }

    pub fn process(&mut self, input: Vec<f32>) -> Vec<f32> {
        let delays_len = self.delays.len();
        let input_len = input.len();
        let mut output = HouseholderFDN::split(input, delays_len);

        // Run the delay lines
        for (ii, sample) in output.iter_mut().enumerate() {
            let input = *sample + self.values[ii];
            *sample = self.filters[ii].tick(self.delays[ii].tick(input)) * self.gain;
        }

        // Householder feedback matrix. All outputs are summed and fed back into all inputs
        // https://github.com/madronalabs/madronalib/blob/master/source/DSP/MLDSPFilters.h#L953
        // https://ccrma.stanford.edu/~jos/pasp/Householder_Feedback_Matrix.html
        let mut delay_sum: f32 = output.iter().sum();
        delay_sum *= 2.0 / delays_len as f32;

        // Set the feedback, all delays are fed back into each other
        for (ii, value) in self.values.iter_mut().enumerate() {
            *value = output[ii] - delay_sum;
        }

        HouseholderFDN::join(output, input_len)
    }

    pub fn set_gain(&mut self, gain: f32) -> () {
        self.gain = gain;
    }

    pub fn set_delays(&mut self, delays: Vec<usize>) -> () {
        for (ii, delay) in delays.iter().enumerate() {
            self.delays[ii].set_delay(*delay);
        }
    }

    pub fn set_max_delays(&mut self, max_delay: usize) -> () {
        for delay in self.delays.iter_mut() {
            delay.set_max_delay(max_delay);
        }
    }

    pub fn set_cutoff(&mut self, cutoff: f32) -> () {
        for filter in self.filters.iter_mut() {
            filter.set_cutoff(cutoff);
        }
    }
}

pub fn get_max_float(values: &Vec<f32>) -> f32 {
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));

    sorted[0]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_fdn() {
        let mut fdn = HouseholderFDN::new(vec![2, 3, 5, 7], 0.5, 10);

        for _i in 0..10 {
            fdn.process(vec![1.0, 1.0]);
        }

        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.296875, 0.3125]);
        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.25390625, 0.296875]);
        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.31640625, 0.328125]);
        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.30859375, 0.171875]);
    }

    #[test]
    fn test_fdn_lowpass() {
        let mut fdn = HouseholderFDN::new(vec![2, 3, 5, 7], 1.0, 10);

        fdn.set_cutoff(0.09);

        for _i in 0..10 {
            fdn.process(vec![1.0, 1.0]);
        }

        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.70215225, 0.64007735]);
        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.52303684, 0.52741337]);
        assert_eq!(fdn.process(vec![1.0, 1.0]), [0.41039184, 0.44365278]);
    }

    #[test]
    fn test_sort() {
        assert_eq!(get_max_float(&vec![0.1, 0.2, 0.3]), 0.3);
    }
}
