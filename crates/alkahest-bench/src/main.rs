use std::path::PathBuf;
use std::process;

use alkahest_bench::report;
use alkahest_bench::runner::BenchmarkRunner;
use alkahest_bench::scenes;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

    let mut baseline_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut regression_threshold = 10.0f64;
    let mut tick_count = 120u32;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline_path = Some(PathBuf::from(&args[i]));
            }
            "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--regression-threshold" => {
                i += 1;
                regression_threshold = args[i]
                    .parse()
                    .expect("invalid --regression-threshold value");
            }
            "--ticks" => {
                i += 1;
                tick_count = args[i].parse().expect("invalid --ticks value");
            }
            "--help" | "-h" => {
                eprintln!("Usage: bench-runner [OPTIONS]");
                eprintln!("  --baseline <path>              Load baseline JSON for comparison");
                eprintln!("  --output <path>                Save current results as JSON baseline");
                eprintln!(
                    "  --regression-threshold <pct>   Regression threshold percentage (default: 10)"
                );
                eprintln!("  --ticks <n>                    Ticks per scene (default: 120)");
                process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    log::info!("Initializing GPU...");
    let runner = BenchmarkRunner::new(tick_count);

    let scene_configs = scenes::standard_scenes();
    let mut results = Vec::new();

    for config in &scene_configs {
        let result = runner.run_scene(config);
        results.push(result);
    }

    // Print markdown summary
    println!("\n## Benchmark Results\n");
    println!("{}", report::format_markdown(&results));

    // Save output baseline
    if let Some(ref path) = output_path {
        let baseline = report::Baseline {
            timestamp: chrono_timestamp(),
            results: results.clone(),
        };
        report::save_baseline(path, &baseline).expect("failed to save baseline");
        log::info!("Saved baseline to {}", path.display());
    }

    // Compare against baseline
    if let Some(ref path) = baseline_path {
        if let Some(baseline) = report::load_baseline(path) {
            let regressions = report::compare(&results, &baseline, regression_threshold);
            println!(
                "{}",
                report::format_comparison(&regressions, regression_threshold)
            );
            if !regressions.is_empty() {
                eprintln!(
                    "ERROR: {} regressions detected, exiting with code 1",
                    regressions.len()
                );
                process::exit(1);
            }
        } else {
            log::warn!("Baseline file not found: {}", path.display());
        }
    }

    log::info!("Benchmark complete.");
}

/// Simple timestamp without chrono dependency.
fn chrono_timestamp() -> String {
    // Use a fixed format since we don't want to add a chrono dependency
    format!("bench-{}", std::process::id())
}
