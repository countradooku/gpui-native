use std::fmt::Write as _;
use std::time::{Duration, Instant};

use gpui_vue_core::{GpuiRenderer, WindowOptions};

const NODE_COUNT: usize = 10_000;
const ROUNDS: usize = 11;

fn mutation_batch(value: &str) -> String {
    let mut json = String::with_capacity(NODE_COUNT * 34);
    json.push('[');
    for index in 0..NODE_COUNT {
        if index > 0 {
            json.push(',');
        }
        write!(json, "[\"setText\",{},\"{value}\"]", index + 2).unwrap();
    }
    json.push(']');
    json
}

fn style_batch(color: &str, display: &str) -> String {
    let mut json = String::with_capacity(NODE_COUNT * 74);
    json.push('[');
    for index in 0..NODE_COUNT {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "[\"setStyle\",{},{{\"color\":\"{color}\",\"display\":\"{display}\",\"fontWeight\":600}}]",
            index + 2
        )
        .unwrap();
    }
    json.push(']');
    json
}

fn initial_batch() -> String {
    let mut json = String::with_capacity(NODE_COUNT * 70);
    json.push_str("[[\"createElement\",1,\"div\"],[\"setRoot\",1]");
    for index in 0..NODE_COUNT {
        let id = index + 2;
        write!(
            json,
            ",[\"createElement\",{id},\"text\"],[\"setText\",{id},\"warm\"],[\"appendChild\",1,{id}]"
        )
        .unwrap();
    }
    json.push(']');
    json
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn measure(renderer: &GpuiRenderer, batches: [&str; 2]) -> Duration {
    for warmup in 0..2 {
        renderer
            .apply_batch(batches[warmup % 2].to_owned())
            .unwrap();
    }
    let mut samples = Vec::with_capacity(ROUNDS);
    for round in 0..ROUNDS {
        let started = Instant::now();
        renderer.apply_batch(batches[round % 2].to_owned()).unwrap();
        samples.push(started.elapsed());
    }
    median(samples)
}

fn operations_per_second(duration: Duration) -> f64 {
    f64::from(u32::try_from(NODE_COUNT).unwrap()) / duration.as_secs_f64()
}

fn main() {
    let renderer = GpuiRenderer::new(None);
    let options = WindowOptions {
        headless: Some(true),
        ..WindowOptions::default()
    };
    renderer.init(Some(options)).unwrap();
    renderer.apply_batch(initial_batch()).unwrap();

    let text_a = mutation_batch("alpha");
    let text_b = mutation_batch("bravo");
    let text_duration = measure(&renderer, [&text_a, &text_b]);

    let style_a = style_batch("#f8fafc", "flex");
    let style_b = style_batch("#0f172a", "block");
    let style_duration = measure(&renderer, [&style_a, &style_b]);

    let text_rate = operations_per_second(text_duration);
    let style_rate = operations_per_second(style_duration);
    assert!(
        text_rate >= 50_000.0,
        "text batch throughput regressed: {text_rate:.0}/s"
    );
    assert!(
        style_rate >= 50_000.0,
        "style batch throughput regressed: {style_rate:.0}/s"
    );

    println!("\nRust retained-tree benchmark (10,000 mutations/batch, release median)");
    println!(
        "text batch:  {:>8.3} ms  {:>12.0} mutations/s",
        text_duration.as_secs_f64() * 1_000.0,
        text_rate
    );
    println!(
        "style batch: {:>8.3} ms  {:>12.0} mutations/s",
        style_duration.as_secs_f64() * 1_000.0,
        style_rate
    );
    renderer.close().unwrap();
}
