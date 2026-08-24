//! Bounded interleaved-f32 audio queue for decoded-frame producers.
//!
//! The N-API surface accepts a Float32Array in one call per decoded chunk.
//! Old data is dropped on overflow, which bounds memory and favors low latency.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct AudioQueueSnapshot {
    pub sample_rate: u32,
    pub channels: u32,
    pub capacity_frames: usize,
    pub queued_frames: usize,
    pub dropped_frames: u64,
}

pub struct AudioFrameQueue {
    sample_rate: u32,
    channels: u32,
    capacity_frames: usize,
    samples: VecDeque<f32>,
    dropped_frames: u64,
}

impl Default for AudioFrameQueue {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            capacity_frames: 96_000,
            samples: VecDeque::with_capacity(192_000),
            dropped_frames: 0,
        }
    }
}

impl AudioFrameQueue {
    pub fn configure(&mut self, sample_rate: u32, channels: u32, capacity_frames: usize) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.capacity_frames = capacity_frames;
        self.samples = VecDeque::with_capacity(capacity_frames.saturating_mul(channels as usize));
        self.dropped_frames = 0;
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<(), String> {
        let channels = self.channels as usize;
        if samples.len() % channels != 0 {
            return Err(format!(
                "audio sample count {} is not divisible by {} channels",
                samples.len(),
                channels
            ));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err("audio samples must be finite f32 PCM values".to_string());
        }
        let capacity_samples = self.capacity_frames.saturating_mul(channels);
        let retained = samples.len().min(capacity_samples);
        let incoming = &samples[samples.len() - retained..];
        let dropped_incoming = samples.len() - retained;
        let overflow = self
            .samples
            .len()
            .saturating_add(retained)
            .saturating_sub(capacity_samples);
        let drop_samples = overflow.min(self.samples.len());
        self.samples.drain(..drop_samples);
        self.dropped_frames = self
            .dropped_frames
            .saturating_add(((dropped_incoming + drop_samples) / channels) as u64);
        self.samples
            .extend(incoming.iter().map(|sample| sample.clamp(-1.0, 1.0)));
        Ok(())
    }

    pub fn pop(&mut self, max_frames: usize) -> Vec<f32> {
        let count = max_frames
            .min(self.samples.len() / self.channels as usize)
            .saturating_mul(self.channels as usize);
        self.samples.drain(..count).collect()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn snapshot(&self) -> AudioQueueSnapshot {
        AudioQueueSnapshot {
            sample_rate: self.sample_rate,
            channels: self.channels,
            capacity_frames: self.capacity_frames,
            queued_frames: self.samples.len() / self.channels as usize,
            dropped_frames: self.dropped_frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_keeps_the_newest_complete_frames() {
        let mut queue = AudioFrameQueue::default();
        queue.configure(48_000, 2, 2);
        queue.push(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5]).unwrap();
        assert_eq!(queue.pop(2), vec![0.2, 0.3, 0.4, 0.5]);
        assert_eq!(queue.snapshot().dropped_frames, 1);
    }

    #[test]
    fn rejects_partial_and_non_finite_frames() {
        let mut queue = AudioFrameQueue::default();
        assert!(queue.push(&[0.0]).is_err());
        assert!(queue.push(&[0.0, f32::NAN]).is_err());
    }
}
