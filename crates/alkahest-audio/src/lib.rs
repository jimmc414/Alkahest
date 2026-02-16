pub mod scanner;

#[cfg(target_arch = "wasm32")]
pub mod bridge;
#[cfg(target_arch = "wasm32")]
pub mod generators;
#[cfg(target_arch = "wasm32")]
pub mod mixer;

pub use scanner::{AudioCategory, AudioSource};

/// Top-level audio facade. Owns the scanner (platform-independent) and
/// the Web Audio mixer (WASM-only). When disabled, update is a no-op
/// with zero CPU cost.
pub struct AudioSystem {
    scanner: scanner::AudioScanner,
    #[cfg(target_arch = "wasm32")]
    mixer: Option<mixer::AudioMixer>,
    enabled: bool,
    volume: f32,
}

impl Default for AudioSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSystem {
    /// Create a new AudioSystem. The mixer is lazily initialized when
    /// `set_enabled(true)` is called (Web Audio requires a user gesture).
    pub fn new() -> Self {
        Self {
            scanner: scanner::AudioScanner::new(),
            #[cfg(target_arch = "wasm32")]
            mixer: None,
            enabled: false,
            volume: 0.7,
        }
    }

    /// Enable or disable the audio system.
    /// When enabling: creates mixer (lazy init) and resumes AudioContext.
    /// When disabling: stops all sounds, drops mixer, clears scanner.
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled == self.enabled {
            return;
        }
        self.enabled = enabled;

        #[cfg(target_arch = "wasm32")]
        {
            if enabled {
                match mixer::AudioMixer::new() {
                    Ok(m) => {
                        m.resume();
                        m.set_volume(self.volume);
                        self.mixer = Some(m);
                        log::info!("Audio system enabled");
                    }
                    Err(e) => {
                        log::error!("Failed to create audio mixer: {:?}", e);
                        self.enabled = false;
                    }
                }
            } else {
                if let Some(ref mut m) = self.mixer {
                    m.stop_all();
                }
                self.mixer = None;
                log::info!("Audio system disabled");
            }
        }

        if !enabled {
            self.scanner.clear();
        }
    }

    /// Check if audio is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the master volume (0.0-1.0).
    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        #[cfg(target_arch = "wasm32")]
        if let Some(ref m) = self.mixer {
            m.set_volume(self.volume);
        }
    }

    /// Register an audio event. No-op when disabled.
    pub fn register_event(
        &mut self,
        position: glam::Vec3,
        category: scanner::AudioCategory,
        intensity: f32,
    ) {
        if !self.enabled {
            return;
        }
        self.scanner.register_event(position, category, intensity);
    }

    /// Update the audio system. When disabled: immediate return (zero CPU cost).
    /// When enabled: scanner produces sources, mixer spatializes them.
    pub fn update(
        &mut self,
        dt_secs: f32,
        camera_pos: glam::Vec3,
        camera_forward: glam::Vec3,
        active_chunk_count: u32,
    ) {
        if !self.enabled {
            return;
        }

        let sources = self.scanner.update(dt_secs, active_chunk_count);

        #[cfg(target_arch = "wasm32")]
        if let Some(ref mut m) = self.mixer {
            m.update(camera_pos, camera_forward, &sources);
        }

        // Suppress unused variable warning on native builds
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (camera_pos, camera_forward, &sources);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_system_disabled_noop() {
        let mut sys = AudioSystem::new();
        assert!(!sys.is_enabled());

        // Register event while disabled — no panic
        sys.register_event(glam::Vec3::ZERO, AudioCategory::Fire, 1.0);

        // Update while disabled — no panic, immediate return
        sys.update(0.016, glam::Vec3::ZERO, glam::Vec3::Z, 8);
    }

    #[test]
    fn test_enabled_toggle() {
        let mut sys = AudioSystem::new();
        assert!(!sys.is_enabled());

        // On native, set_enabled(true) sets enabled flag but mixer stays None
        sys.set_enabled(true);
        // On native (non-WASM), enabled stays true but mixer is None
        #[cfg(not(target_arch = "wasm32"))]
        assert!(sys.is_enabled());

        sys.set_enabled(false);
        assert!(!sys.is_enabled());
    }

    #[test]
    fn test_volume_clamp() {
        let mut sys = AudioSystem::new();
        sys.set_volume(1.5);
        assert!((sys.volume - 1.0).abs() < f32::EPSILON);
        sys.set_volume(-0.5);
        assert!(sys.volume.abs() < f32::EPSILON);
        sys.set_volume(0.5);
        assert!((sys.volume - 0.5).abs() < f32::EPSILON);
    }
}
