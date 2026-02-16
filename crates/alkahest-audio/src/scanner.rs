/// Categories of audio events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCategory {
    Fire,
    Water,
    Steam,
    Explosion,
    Collapse,
}

impl AudioCategory {
    /// Default time-to-live in seconds for each category.
    fn default_ttl(self) -> f32 {
        match self {
            AudioCategory::Fire => 3.0,
            AudioCategory::Water => 2.0,
            AudioCategory::Steam => 2.0,
            AudioCategory::Explosion => 1.5,
            AudioCategory::Collapse => 2.0,
        }
    }

    /// Map a material ID to an audio category, if applicable.
    pub fn from_material_id(mat_id: u32) -> Option<Self> {
        match mat_id {
            5..=7 => Some(AudioCategory::Fire), // Fire, Ember, Spark
            3 => Some(AudioCategory::Water),
            174 => Some(AudioCategory::Steam),
            _ => None,
        }
    }
}

/// A positioned audio source with intensity.
#[derive(Debug, Clone)]
pub struct AudioSource {
    pub position: glam::Vec3,
    pub category: AudioCategory,
    pub intensity: f32, // 0.0-1.0
}

/// Tracks audio-relevant events from tool actions and world state.
pub struct AudioScanner {
    /// Active audio events with decay timers.
    events: Vec<TrackedEvent>,
    /// Maximum tracked events (prevents unbounded growth).
    max_events: usize,
}

struct TrackedEvent {
    source: AudioSource,
    ttl: f32, // seconds remaining
}

impl Default for AudioScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioScanner {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            max_events: 64,
        }
    }

    /// Register a new audio event at the given position.
    pub fn register_event(
        &mut self,
        position: glam::Vec3,
        category: AudioCategory,
        intensity: f32,
    ) {
        let ttl = category.default_ttl();
        let source = AudioSource {
            position,
            category,
            intensity: intensity.clamp(0.0, 1.0),
        };
        self.events.push(TrackedEvent { source, ttl });

        // Cap event count: remove oldest events first
        while self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    /// Advance timers, remove expired events, and return current audio sources.
    /// `active_chunk_count` scales ambient intensity for continuous sounds.
    pub fn update(&mut self, dt_secs: f32, active_chunk_count: u32) -> Vec<AudioSource> {
        // Decrement TTLs and remove expired
        for event in &mut self.events {
            event.ttl -= dt_secs;
        }
        self.events.retain(|e| e.ttl > 0.0);

        // Scale intensity based on remaining TTL (fade out) and active chunks
        let chunk_scale = (active_chunk_count as f32 / 8.0).clamp(0.1, 1.0);

        self.events
            .iter()
            .map(|e| {
                let fade = (e.ttl / e.source.category.default_ttl()).clamp(0.0, 1.0);
                AudioSource {
                    position: e.source.position,
                    category: e.source.category,
                    intensity: e.source.intensity * fade * chunk_scale,
                }
            })
            .collect()
    }

    /// Remove all tracked events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_update() {
        let mut scanner = AudioScanner::new();
        scanner.register_event(glam::Vec3::new(10.0, 5.0, 10.0), AudioCategory::Fire, 1.0);

        let sources = scanner.update(0.0, 8);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].category, AudioCategory::Fire);
        assert!((sources[0].position.x - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_event_decay() {
        let mut scanner = AudioScanner::new();
        scanner.register_event(glam::Vec3::ZERO, AudioCategory::Explosion, 1.0);

        // Advance past explosion TTL (1.5s)
        let sources = scanner.update(2.0, 8);
        assert!(sources.is_empty(), "Expired event should be removed");
    }

    #[test]
    fn test_max_events_cap() {
        let mut scanner = AudioScanner::new();
        for i in 0..100 {
            scanner.register_event(
                glam::Vec3::new(i as f32, 0.0, 0.0),
                AudioCategory::Fire,
                1.0,
            );
        }
        // Scanner caps at 64
        let sources = scanner.update(0.0, 8);
        assert!(
            sources.len() <= 64,
            "Should be capped at max_events, got {}",
            sources.len()
        );
    }

    #[test]
    fn test_clear_removes_all() {
        let mut scanner = AudioScanner::new();
        scanner.register_event(glam::Vec3::ZERO, AudioCategory::Fire, 1.0);
        scanner.register_event(glam::Vec3::ZERO, AudioCategory::Water, 1.0);
        scanner.clear();
        let sources = scanner.update(0.0, 8);
        assert!(sources.is_empty());
    }

    #[test]
    fn test_multiple_categories() {
        let mut scanner = AudioScanner::new();
        scanner.register_event(glam::Vec3::new(1.0, 0.0, 0.0), AudioCategory::Fire, 1.0);
        scanner.register_event(glam::Vec3::new(2.0, 0.0, 0.0), AudioCategory::Water, 0.8);
        scanner.register_event(
            glam::Vec3::new(3.0, 0.0, 0.0),
            AudioCategory::Explosion,
            0.5,
        );

        let sources = scanner.update(0.0, 8);
        assert_eq!(sources.len(), 3);

        let categories: Vec<AudioCategory> = sources.iter().map(|s| s.category).collect();
        assert!(categories.contains(&AudioCategory::Fire));
        assert!(categories.contains(&AudioCategory::Water));
        assert!(categories.contains(&AudioCategory::Explosion));
    }

    #[test]
    fn test_intensity_scaling() {
        let mut scanner = AudioScanner::new();
        scanner.register_event(glam::Vec3::ZERO, AudioCategory::Fire, 1.0);

        // Low active chunk count should scale down intensity
        let sources_low = scanner.update(0.0, 1);
        let intensity_low = sources_low[0].intensity;

        // Re-register since update consumed time
        let mut scanner2 = AudioScanner::new();
        scanner2.register_event(glam::Vec3::ZERO, AudioCategory::Fire, 1.0);
        let sources_high = scanner2.update(0.0, 16);
        let intensity_high = sources_high[0].intensity;

        assert!(
            intensity_low < intensity_high,
            "Low chunk count ({}) should produce lower intensity than high ({})",
            intensity_low,
            intensity_high
        );
    }

    #[test]
    fn test_material_id_mapping() {
        assert_eq!(
            AudioCategory::from_material_id(5),
            Some(AudioCategory::Fire)
        );
        assert_eq!(
            AudioCategory::from_material_id(6),
            Some(AudioCategory::Fire)
        );
        assert_eq!(
            AudioCategory::from_material_id(7),
            Some(AudioCategory::Fire)
        );
        assert_eq!(
            AudioCategory::from_material_id(3),
            Some(AudioCategory::Water)
        );
        assert_eq!(
            AudioCategory::from_material_id(174),
            Some(AudioCategory::Steam)
        );
        assert_eq!(AudioCategory::from_material_id(1), None); // Stone
        assert_eq!(AudioCategory::from_material_id(0), None); // Air
    }
}
