use crate::bridge::AudioBridge;
use crate::generators::{self, SoundGenerator};
use crate::scanner::{AudioCategory, AudioSource};
use wasm_bindgen::prelude::*;

/// A slot in the mixer holding an active sound instance.
struct MixerSlot {
    category: AudioCategory,
    position: glam::Vec3,
    generator: Box<dyn SoundGenerator>,
}

/// Spatial audio mixer. Maps AudioSources to Web Audio generators
/// with distance attenuation and stereo panning.
pub struct AudioMixer {
    bridge: AudioBridge,
    active_sounds: Vec<MixerSlot>,
    max_simultaneous: usize,
}

impl AudioMixer {
    /// Create a new mixer. Call `bridge.resume()` after a user gesture.
    pub fn new() -> Result<Self, JsValue> {
        let bridge = AudioBridge::new()?;
        Ok(Self {
            bridge,
            active_sounds: Vec::new(),
            max_simultaneous: 24,
        })
    }

    /// Update the mixer with current audio sources relative to the camera.
    pub fn update(
        &mut self,
        camera_pos: glam::Vec3,
        camera_forward: glam::Vec3,
        sources: &[AudioSource],
    ) {
        // Compute camera right vector for stereo panning
        let up = glam::Vec3::Y;
        let camera_right = camera_forward.cross(up).normalize_or_zero();

        // Score each source by effective volume
        let mut scored: Vec<(usize, f32)> = sources
            .iter()
            .enumerate()
            .map(|(i, src)| {
                let distance = (src.position - camera_pos).length();
                let attenuation = 1.0 / (1.0 + distance / 32.0);
                let effective = src.intensity * attenuation;
                (i, effective)
            })
            .collect();

        // Sort by descending effective volume
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Keep only top N sources
        scored.truncate(self.max_simultaneous);

        // Determine which existing slots to keep, update, or stop
        let desired_count = scored.len();

        // Stop generators beyond the desired count
        while self.active_sounds.len() > desired_count {
            if let Some(mut slot) = self.active_sounds.pop() {
                slot.generator.stop();
            }
        }

        // Update existing slots and add new ones
        for (slot_idx, &(src_idx, effective_volume)) in scored.iter().enumerate() {
            let source = &sources[src_idx];

            // Compute stereo pan: dot of direction to source with camera right
            let dir = (source.position - camera_pos).normalize_or_zero();
            let _pan = dir.dot(camera_right).clamp(-1.0, 1.0);

            if slot_idx < self.active_sounds.len() {
                // Update existing slot
                let slot = &mut self.active_sounds[slot_idx];
                if slot.category == source.category {
                    // Same category: just update intensity
                    slot.generator.update_intensity(effective_volume);
                    slot.position = source.position;
                } else {
                    // Different category: stop old, start new
                    slot.generator.stop();
                    let mut gen = generators::create_generator(source.category);
                    gen.start(&self.bridge, effective_volume);
                    slot.category = source.category;
                    slot.position = source.position;
                    slot.generator = gen;
                }
            } else {
                // New slot
                let mut gen = generators::create_generator(source.category);
                gen.start(&self.bridge, effective_volume);
                self.active_sounds.push(MixerSlot {
                    category: source.category,
                    position: source.position,
                    generator: gen,
                });
            }
        }
    }

    /// Set the master volume (0.0-1.0).
    pub fn set_volume(&self, vol: f32) {
        self.bridge.set_master_volume(vol);
    }

    /// Resume the AudioContext (required after user gesture).
    pub fn resume(&self) {
        self.bridge.resume();
    }

    /// Stop all active generators and clear slots.
    pub fn stop_all(&mut self) {
        for slot in &mut self.active_sounds {
            slot.generator.stop();
        }
        self.active_sounds.clear();
    }
}
