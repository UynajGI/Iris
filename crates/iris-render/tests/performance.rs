// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in synthetic managed-allocation smoke, not RSS/VRAM measurements.
//! IRIS_LARGE_MEMORY_SMOKE=1 cargo test -p iris-render --test performance -- --ignored --nocapture --test-threads=1
//! Provision >= 4 GiB free RAM; cases are sequential, at most two 61 MP float frames.
use iris_render::{
    adjustments::{self, Adjustments},
    Frame, RenderContext,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires explicit 4 GiB free-memory provisioning; run serially"]
fn synthetic_45_61mp_managed_budget_and_cancel_release() {
    assert_eq!(
        std::env::var("IRIS_LARGE_MEMORY_SMOKE").as_deref(),
        Ok("1"),
        "explicit large-memory opt-in required"
    );
    for (width, height) in [(9000, 5000), (10000, 6100)] {
        exercise(width, height);
    }
}
fn exercise(width: u32, height: u32) {
    let bytes = u64::from(width) * u64::from(height) * 16;
    let context = RenderContext::new(bytes * 2);
    let source_reservation = context.reserve(bytes).unwrap();
    let source = Arc::new(
        Frame::new(
            width,
            height,
            vec![[0.125, 0.25, 2.0, 1.0]; (u64::from(width) * u64::from(height)) as usize],
        )
        .unwrap(),
    );
    let parameters = Adjustments {
        exposure: 1.0,
        ..Default::default()
    };
    let start = Instant::now();
    let output = adjustments::apply(&source, &parameters, &context).unwrap();
    assert_eq!(output.pixels[0], [0.25, 0.5, 4.0, 1.0]);
    drop(output);
    assert_eq!(context.memory_used(), bytes);
    let denied = RenderContext::new(bytes * 2 - 1);
    let input_guard = denied.reserve(bytes).unwrap();
    assert!(adjustments::apply(&source, &parameters, &denied).is_err());
    drop(input_guard);
    assert_eq!(denied.memory_used(), 0);
    let worker_context = context.clone();
    let worker_source = source.clone();
    let worker = std::thread::spawn(move || {
        adjustments::apply(&worker_source, &parameters, &worker_context)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while context.memory_used() < bytes * 2 && !worker.is_finished() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    let observed = context.memory_used();
    context.cancel();
    let cancelled = worker.join().unwrap();
    assert!(
        cancelled.is_err(),
        "operation finished before cancellation could be observed"
    );
    assert_eq!(
        observed,
        bytes * 2,
        "did not observe admitted operation scratch"
    );
    assert_eq!(context.memory_used(), bytes);
    drop(source);
    drop(source_reservation);
    assert_eq!(context.memory_used(), 0);
    println!("width={width} height={height} managed_limit={} observed_managed_peak={observed} final_managed=0 elapsed_ms={} scope=synthetic_cpu_debug_not_rss",bytes*2,start.elapsed().as_millis());
}
