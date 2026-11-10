//! Rolling FPS average with spike rejection (port of `FpsCalc.cs`).

const FPS_LEN: usize = 5;

#[derive(Debug, Default)]
pub struct FpsCalc {
    stat: [f32; FPS_LEN],
    index: Option<usize>,
    avg: i32,
    ignored_spikes: u32,
}

impl FpsCalc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn average(&mut self, new_value: i32) -> i32 {
        let mut new_value = new_value as f32;
        match self.index {
            None => {
                self.stat = [new_value; FPS_LEN];
                self.avg = new_value.round() as i32;
                self.index = Some(1);
            }
            Some(idx) => {
                let avg = self.avg as f32;
                let is_spike = (new_value - avg).abs() / avg > 0.1;
                if self.ignored_spikes != 3 && is_spike {
                    self.ignored_spikes += 1;
                    return self.avg;
                }
                self.ignored_spikes = 0;

                let delta = new_value - avg;
                if delta < 0.0 && delta.abs() > avg * 0.1 {
                    new_value += delta.abs() / 2.0;
                }

                self.stat[idx] = new_value;
                let next = if idx + 1 >= FPS_LEN { 0 } else { idx + 1 };
                self.index = Some(next);
                // C# Convert.ToInt32 rounds half to even
                self.avg = round_half_even(self.stat.iter().sum::<f32>() / FPS_LEN as f32);
            }
        }
        self.avg
    }

    pub fn reset(&mut self) {
        self.index = None;
        self.avg = 0;
    }
}

fn round_half_even(v: f32) -> i32 {
    let r = v.round();
    if (v - v.trunc()).abs() == 0.5 && (r as i32) % 2 != 0 {
        (r - v.signum()) as i32
    } else {
        r as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_value_initialises_average() {
        let mut f = FpsCalc::new();
        assert_eq!(f.average(60), 60);
    }

    #[test]
    fn small_changes_move_average() {
        let mut f = FpsCalc::new();
        f.average(60);
        assert_eq!(f.average(63), 61); // (60*4 + 63)/5 = 60.6
    }

    #[test]
    fn spikes_are_ignored_three_times_then_accepted() {
        let mut f = FpsCalc::new();
        f.average(60);
        assert_eq!(f.average(120), 60);
        assert_eq!(f.average(120), 60);
        assert_eq!(f.average(120), 60);
        assert_ne!(f.average(120), 60);
    }

    #[test]
    fn reset_reinitialises() {
        let mut f = FpsCalc::new();
        f.average(60);
        f.reset();
        assert_eq!(f.average(30), 30);
    }

    #[test]
    fn half_even_rounding() {
        assert_eq!(round_half_even(60.5), 60);
        assert_eq!(round_half_even(61.5), 62);
        assert_eq!(round_half_even(60.4), 60);
    }
}
