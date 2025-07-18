//! Tool for generating diff of js-framework-benchmark runs between two sycamore versions.

use std::collections::BTreeMap;
use std::{env, fs};

use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Result {
    framework: String,
    benchmark: String,
    values: ResultValues,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ResultValues {
    Run {
        #[allow(dead_code)]
        total: Vec<f64>,
        script: Vec<f64>,
    },
    Memory {
        #[serde(rename = "DEFAULT")]
        default: Vec<f64>,
    },
}

#[derive(Default)]
struct BenchmarkResults {
    bindgen: f64,
    baseline: f64,
    update: f64,
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let results_path = args
        .get(1)
        .expect("path to results file should be passed as an argument");
    let result_str = fs::read_to_string(results_path).expect("could not read results file");

    let results: Vec<Result> = serde_json::from_str(&result_str).expect("failed to deserialize");
    let mut benchmark_results: BTreeMap<String, BenchmarkResults> = BTreeMap::new();
    for result in results.into_iter() {
        let values = match result.values {
            ResultValues::Run { total: _, script } => script,
            ResultValues::Memory { default } => default,
        };
        let value_sum: f64 = values.iter().sum();
        let value_count = values.len();
        let avg_val = if value_count > 0 {
            value_sum / value_count as f64
        } else {
            0f64
        };

        let entry = benchmark_results.entry(result.benchmark).or_default();
        if result.framework.starts_with("wasm-bindgen") {
            entry.bindgen = avg_val
        } else if result.framework.starts_with("sycamore-baseline") {
            entry.baseline = avg_val;
        } else if result.framework.starts_with("sycamore") {
            entry.update = avg_val;
        }
    }

    let max_benchmark_name_len = benchmark_results.keys().map(|key| key.len()).max().unwrap();
    let full_length = max_benchmark_name_len + 35 + 4 * 3 + 2; // 35: sum of columns defined below, 3: padding, 2: sign

    println!("### Benchmark Report");
    println!("- `wasm-bindgen`: the performance goal");
    println!("- `baseline`: performance of `sycamore-baseline` (typically latest main)");
    println!("- `update`: performance of `sycamore` (typically recent changes)");
    println!("- `diff`: measures the improvement of `update` over the `baseline`");
    println!("```diff");
    println!("@@ {:^1$} @@", "Performance Diff", full_length - 6);
    println!();
    println!(
        "##{:>1$} | wasm-bindgen | baseline |  update |  diff ##",
        "", max_benchmark_name_len
    );
    println!("{}", "#".repeat(full_length));
    for (benchmark, results) in benchmark_results {
        let diff = (results.update - results.baseline) / results.baseline; // TODO zero check
        let sign = if diff < -0.03 {
            "+"
        } else if diff > 0.03 {
            "-"
        } else {
            " "
        };
        print!("{sign} {benchmark:<max_benchmark_name_len$} | ");
        print!("{:>1$} | ", format!("{:.2}", results.bindgen), 12); // 12: wasm-bindgen
        print!("{:>1$} | ", format!("{:.2}", results.baseline), 8); // 8: baseline
        print!("{:>1$} | ", format!("{:.2}", results.update), 7); // 7: f64 spacing
        println!("{:>1$}", format!("{:+.2}%", 100f64 * diff), 8); // 8: pct spacing
    }
    println!("```");
}
