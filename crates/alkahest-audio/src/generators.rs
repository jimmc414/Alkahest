use crate::bridge::AudioBridge;
use wasm_bindgen::prelude::*;
use web_sys::{AudioBufferSourceNode, BiquadFilterType, GainNode, OscillatorNode, OscillatorType};

/// Trait for procedural sound generators.
pub trait SoundGenerator {
    fn start(&mut self, bridge: &AudioBridge, intensity: f32);
    fn update_intensity(&mut self, intensity: f32);
    fn stop(&mut self);
    fn is_active(&self) -> bool;
}

/// Fire sound: noise -> bandpass filter -> gain. Crackling effect.
#[derive(Default)]
pub struct FireGenerator {
    source: Option<AudioBufferSourceNode>,
    filter: Option<web_sys::BiquadFilterNode>,
    gain: Option<GainNode>,
    active: bool,
}

impl FireGenerator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SoundGenerator for FireGenerator {
    fn start(&mut self, bridge: &AudioBridge, intensity: f32) {
        if self.active {
            return;
        }
        let result: Result<(), JsValue> = (|| {
            let (source, gain_node) = bridge.create_noise_source(0.0)?;
            let filter = bridge.create_filter(BiquadFilterType::Bandpass, 1200.0, 1.5)?;

            // Re-route: source -> filter -> gain -> master
            source.disconnect()?;
            source.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&gain_node)?;

            gain_node.gain().set_value(intensity * 0.3);
            #[allow(deprecated)]
            source.start()?;

            self.source = Some(source);
            self.filter = Some(filter);
            self.gain = Some(gain_node);
            self.active = true;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("FireGenerator start failed: {:?}", e);
        }
    }

    fn update_intensity(&mut self, intensity: f32) {
        if let Some(gain) = &self.gain {
            gain.gain().set_value(intensity * 0.3);
        }
        if let Some(filter) = &self.filter {
            // Modulate filter frequency with intensity for varying crackle
            let freq = 800.0 + intensity * 1200.0;
            filter.frequency().set_value(freq);
        }
    }

    #[allow(deprecated)]
    fn stop(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = source.stop();
        }
        self.filter = None;
        self.gain = None;
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// Water sound: noise -> lowpass filter -> gain. Smooth flow.
#[derive(Default)]
pub struct WaterGenerator {
    source: Option<AudioBufferSourceNode>,
    gain: Option<GainNode>,
    active: bool,
}

impl WaterGenerator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SoundGenerator for WaterGenerator {
    fn start(&mut self, bridge: &AudioBridge, intensity: f32) {
        if self.active {
            return;
        }
        let result: Result<(), JsValue> = (|| {
            let (source, gain_node) = bridge.create_noise_source(0.0)?;
            let filter = bridge.create_filter(BiquadFilterType::Lowpass, 500.0, 0.7)?;

            // Re-route: source -> filter -> gain -> master
            source.disconnect()?;
            source.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&gain_node)?;

            gain_node.gain().set_value(intensity * 0.25);
            #[allow(deprecated)]
            source.start()?;

            self.source = Some(source);
            self.gain = Some(gain_node);
            self.active = true;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("WaterGenerator start failed: {:?}", e);
        }
    }

    fn update_intensity(&mut self, intensity: f32) {
        if let Some(gain) = &self.gain {
            gain.gain().set_value(intensity * 0.25);
        }
    }

    #[allow(deprecated)]
    fn stop(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = source.stop();
        }
        self.gain = None;
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// Steam sound: noise -> highpass filter -> gain. High-frequency hissing.
#[derive(Default)]
pub struct SteamGenerator {
    source: Option<AudioBufferSourceNode>,
    filter: Option<web_sys::BiquadFilterNode>,
    gain: Option<GainNode>,
    active: bool,
}

impl SteamGenerator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SoundGenerator for SteamGenerator {
    fn start(&mut self, bridge: &AudioBridge, intensity: f32) {
        if self.active {
            return;
        }
        let result: Result<(), JsValue> = (|| {
            let (source, gain_node) = bridge.create_noise_source(0.0)?;
            let filter = bridge.create_filter(BiquadFilterType::Highpass, 3000.0, 1.0)?;

            source.disconnect()?;
            source.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&gain_node)?;

            gain_node.gain().set_value(intensity * 0.2);
            #[allow(deprecated)]
            source.start()?;

            self.source = Some(source);
            self.filter = Some(filter);
            self.gain = Some(gain_node);
            self.active = true;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("SteamGenerator start failed: {:?}", e);
        }
    }

    fn update_intensity(&mut self, intensity: f32) {
        if let Some(gain) = &self.gain {
            gain.gain().set_value(intensity * 0.2);
        }
        if let Some(filter) = &self.filter {
            let freq = 2500.0 + intensity * 2000.0;
            filter.frequency().set_value(freq);
        }
    }

    #[allow(deprecated)]
    fn stop(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = source.stop();
        }
        self.filter = None;
        self.gain = None;
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// Explosion sound: low-frequency sawtooth oscillator with exponential decay.
/// One-shot: ramps gain to peak then decays over ~0.5s.
#[derive(Default)]
pub struct ExplosionGenerator {
    osc: Option<OscillatorNode>,
    gain: Option<GainNode>,
    active: bool,
    /// Time when the explosion started (AudioContext time).
    start_time: f64,
}

impl ExplosionGenerator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SoundGenerator for ExplosionGenerator {
    #[allow(deprecated)]
    fn start(&mut self, bridge: &AudioBridge, intensity: f32) {
        if self.active {
            return;
        }
        let result: Result<(), JsValue> = (|| {
            let (osc, gain_node) = bridge.create_oscillator(60.0, OscillatorType::Sawtooth, 0.0)?;

            let now = bridge.current_time();
            // Exponential decay envelope
            gain_node.gain().set_value_at_time(0.0, now)?;
            gain_node
                .gain()
                .linear_ramp_to_value_at_time(intensity * 0.5, now + 0.02)?;
            gain_node
                .gain()
                .exponential_ramp_to_value_at_time(0.001, now + 0.5)?;

            osc.start()?;
            osc.stop_with_when(now + 0.6)?;

            self.osc = Some(osc);
            self.gain = Some(gain_node);
            self.active = true;
            self.start_time = now;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("ExplosionGenerator start failed: {:?}", e);
        }
    }

    fn update_intensity(&mut self, _intensity: f32) {
        // One-shot: envelope is pre-programmed, no live update
    }

    #[allow(deprecated)]
    fn stop(&mut self) {
        if let Some(osc) = self.osc.take() {
            let _ = osc.stop();
        }
        self.gain = None;
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// Collapse sound: low-frequency noise (bandpass 100-300 Hz) with slow decay.
/// Rumbling sound lasting 1-2 seconds.
#[derive(Default)]
pub struct CollapseGenerator {
    source: Option<AudioBufferSourceNode>,
    gain: Option<GainNode>,
    active: bool,
    start_time: f64,
}

impl CollapseGenerator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SoundGenerator for CollapseGenerator {
    #[allow(deprecated)]
    fn start(&mut self, bridge: &AudioBridge, intensity: f32) {
        if self.active {
            return;
        }
        let result: Result<(), JsValue> = (|| {
            let (source, gain_node) = bridge.create_noise_source(0.0)?;
            let filter = bridge.create_filter(BiquadFilterType::Bandpass, 200.0, 0.8)?;

            source.disconnect()?;
            source.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&gain_node)?;

            let now = bridge.current_time();
            gain_node.gain().set_value_at_time(0.0, now)?;
            gain_node
                .gain()
                .linear_ramp_to_value_at_time(intensity * 0.4, now + 0.05)?;
            gain_node
                .gain()
                .exponential_ramp_to_value_at_time(0.001, now + 1.5)?;

            source.start()?;
            source.stop_with_when(now + 2.0)?;

            self.source = Some(source);
            self.gain = Some(gain_node);
            self.active = true;
            self.start_time = now;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("CollapseGenerator start failed: {:?}", e);
        }
    }

    fn update_intensity(&mut self, _intensity: f32) {
        // One-shot: envelope is pre-programmed
    }

    #[allow(deprecated)]
    fn stop(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = source.stop();
        }
        self.gain = None;
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

/// Create a generator for the given audio category.
pub fn create_generator(category: crate::scanner::AudioCategory) -> Box<dyn SoundGenerator> {
    use crate::scanner::AudioCategory;
    match category {
        AudioCategory::Fire => Box::new(FireGenerator::new()),
        AudioCategory::Water => Box::new(WaterGenerator::new()),
        AudioCategory::Steam => Box::new(SteamGenerator::new()),
        AudioCategory::Explosion => Box::new(ExplosionGenerator::new()),
        AudioCategory::Collapse => Box::new(CollapseGenerator::new()),
    }
}
