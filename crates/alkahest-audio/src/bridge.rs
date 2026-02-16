use wasm_bindgen::prelude::*;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, BiquadFilterNode, BiquadFilterType, GainNode,
    OscillatorNode, OscillatorType,
};

/// Wraps the Web Audio API, managing the AudioContext and master gain.
pub struct AudioBridge {
    ctx: AudioContext,
    master_gain: GainNode,
    /// Pre-generated white noise buffer for crackling/hissing effects.
    noise_buffer: AudioBuffer,
}

impl AudioBridge {
    /// Create a new AudioBridge with an AudioContext and master gain node.
    pub fn new() -> Result<Self, JsValue> {
        let ctx = AudioContext::new()?;
        let master_gain = ctx.create_gain()?;
        master_gain.connect_with_audio_node(&ctx.destination())?;
        master_gain.gain().set_value(0.7);

        // Generate 1 second of white noise at the context's sample rate
        let sample_rate = ctx.sample_rate();
        let length = sample_rate as u32;
        let noise_buffer = ctx.create_buffer(1, length, sample_rate)?;
        {
            let mut channel_data = noise_buffer.get_channel_data(0)?;
            // Simple LCG PRNG for deterministic noise (no need for crypto quality)
            let mut seed: u32 = 0xDEAD_BEEF;
            for sample in channel_data.iter_mut() {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                // Convert to [-1.0, 1.0]
                *sample = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            }
            noise_buffer.copy_to_channel(&channel_data, 0)?;
        }

        Ok(Self {
            ctx,
            master_gain,
            noise_buffer,
        })
    }

    /// Set the master volume (0.0-1.0).
    pub fn set_master_volume(&self, volume: f32) {
        self.master_gain.gain().set_value(volume.clamp(0.0, 1.0));
    }

    /// Resume the AudioContext (required after user gesture on most browsers).
    pub fn resume(&self) {
        let _ = self.ctx.resume();
    }

    /// Create an oscillator node connected to a gain node routed to master.
    /// Returns (oscillator, gain_node) so the caller can control both.
    pub fn create_oscillator(
        &self,
        freq: f32,
        osc_type: OscillatorType,
        gain: f32,
    ) -> Result<(OscillatorNode, GainNode), JsValue> {
        let osc = self.ctx.create_oscillator()?;
        osc.set_type(osc_type);
        osc.frequency().set_value(freq);

        let gain_node = self.ctx.create_gain()?;
        gain_node.gain().set_value(gain);

        osc.connect_with_audio_node(&gain_node)?;
        gain_node.connect_with_audio_node(&self.master_gain)?;

        Ok((osc, gain_node))
    }

    /// Create a noise source node (plays the pre-generated noise buffer in a loop).
    /// Returns (source, gain_node).
    pub fn create_noise_source(
        &self,
        gain: f32,
    ) -> Result<(AudioBufferSourceNode, GainNode), JsValue> {
        let source = self.ctx.create_buffer_source()?;
        source.set_buffer(Some(&self.noise_buffer));
        source.set_loop(true);

        let gain_node = self.ctx.create_gain()?;
        gain_node.gain().set_value(gain);

        source.connect_with_audio_node(&gain_node)?;
        gain_node.connect_with_audio_node(&self.master_gain)?;

        Ok((source, gain_node))
    }

    /// Create a biquad filter node connected between two points.
    pub fn create_filter(
        &self,
        filter_type: BiquadFilterType,
        freq: f32,
        q: f32,
    ) -> Result<BiquadFilterNode, JsValue> {
        let filter = self.ctx.create_biquad_filter()?;
        filter.set_type(filter_type);
        filter.frequency().set_value(freq);
        filter.q().set_value(q);
        Ok(filter)
    }

    /// Get the underlying AudioContext for direct access.
    pub fn context(&self) -> &AudioContext {
        &self.ctx
    }

    /// Get the master gain node for routing.
    pub fn master_gain(&self) -> &GainNode {
        &self.master_gain
    }

    /// Get the current audio context time in seconds.
    pub fn current_time(&self) -> f64 {
        self.ctx.current_time()
    }
}
