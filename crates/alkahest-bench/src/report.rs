use std::path::Path;

use crate::runner::BenchmarkResult;

/// A complete baseline containing results from all scenes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Baseline {
    pub timestamp: String,
    pub results: Vec<BenchmarkResult>,
}

/// Load a baseline from a JSON file. Returns None if the file doesn't exist.
pub fn load_baseline(path: &Path) -> Option<Baseline> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save a baseline to a JSON file.
pub fn save_baseline(path: &Path, baseline: &Baseline) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(baseline).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

/// Compare current results against a baseline. Returns a list of regressions
/// (scene name, percent change) where the threshold is exceeded.
pub fn compare(
    current: &[BenchmarkResult],
    baseline: &Baseline,
    threshold_pct: f64,
) -> Vec<(String, f64)> {
    let mut regressions = Vec::new();

    for result in current {
        if let Some(base) = baseline
            .results
            .iter()
            .find(|b| b.scene_name == result.scene_name)
        {
            let pct_change =
                (result.timings.mean_ms - base.timings.mean_ms) / base.timings.mean_ms * 100.0;
            if pct_change > threshold_pct {
                regressions.push((result.scene_name.clone(), pct_change));
            }
        }
    }

    regressions
}

/// Format results as a markdown summary table.
pub fn format_markdown(results: &[BenchmarkResult]) -> String {
    let mut out = String::new();
    out.push_str("| Scene | Voxels | Chunks | Mean (ms) | Median (ms) | P95 (ms) | P99 (ms) | Min (ms) | Max (ms) |\n");
    out.push_str("|-------|--------|--------|-----------|-------------|----------|----------|----------|----------|\n");

    for r in results {
        out.push_str(&format!(
            "| {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            r.scene_name,
            r.active_voxels,
            r.chunk_count,
            r.timings.mean_ms,
            r.timings.median_ms,
            r.timings.p95_ms,
            r.timings.p99_ms,
            r.timings.min_ms,
            r.timings.max_ms,
        ));
    }

    out
}

/// Format a comparison report showing regressions.
pub fn format_comparison(regressions: &[(String, f64)], threshold_pct: f64) -> String {
    if regressions.is_empty() {
        return format!(
            "All scenes within {:.0}% threshold. No regressions detected.\n",
            threshold_pct
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "REGRESSIONS DETECTED (>{:.0}% threshold):\n",
        threshold_pct
    ));
    for (scene, pct) in regressions {
        out.push_str(&format!("  - {}: +{:.1}%\n", scene, pct));
    }
    out
}
