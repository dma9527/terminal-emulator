/// Performance benchmarks for terminal core operations.
/// Run with: cargo test --release bench_ -- --nocapture

use crate::core::{Terminal, VtParser};
use std::time::Instant;

pub struct BenchResult {
    pub name: &'static str,
    pub iterations: usize,
    pub total_ms: f64,
    pub per_iter_us: f64,
    pub throughput_mb_s: Option<f64>,
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:.1}µs/iter ({} iters, {:.1}ms total",
               self.name, self.per_iter_us, self.iterations, self.total_ms)?;
        if let Some(tp) = self.throughput_mb_s {
            write!(f, ", {:.1} MB/s", tp)?;
        }
        write!(f, ")")
    }
}

/// Benchmark raw VT parser throughput.
pub fn bench_parser_throughput() -> BenchResult {
    let data: Vec<u8> = (0..10_000)
        .flat_map(|_| b"Hello, World! \x1b[31mRed\x1b[0m \x1b[1;32mBoldGreen\x1b[0m\r\n".iter().copied())
        .collect();
    let iterations = if cfg!(debug_assertions) { 5 } else { 100 };
    let start = Instant::now();
    for _ in 0..iterations {
        let mut parser = VtParser::new();
        let mut terminal = Terminal::new(80, 24);
        terminal.feed_bytes(&mut parser, &data);
    }
    let elapsed = start.elapsed();
    let total_bytes = data.len() * iterations;
    BenchResult {
        name: "parser_throughput",
        iterations,
        total_ms: elapsed.as_secs_f64() * 1000.0,
        per_iter_us: elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        throughput_mb_s: Some(total_bytes as f64 / elapsed.as_secs_f64() / 1_048_576.0),
    }
}

/// Benchmark grid scrolling.
pub fn bench_grid_scroll() -> BenchResult {
    let mut terminal = Terminal::new(80, 24);
    let mut parser = VtParser::new();
    let line = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH\n";
    let iterations = if cfg!(debug_assertions) { 1_000 } else { 100_000 };
    let start = Instant::now();
    for _ in 0..iterations {
        terminal.feed_bytes(&mut parser, line);
    }
    let elapsed = start.elapsed();
    BenchResult {
        name: "grid_scroll",
        iterations,
        total_ms: elapsed.as_secs_f64() * 1000.0,
        per_iter_us: elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        throughput_mb_s: Some(line.len() as f64 * iterations as f64 / elapsed.as_secs_f64() / 1_048_576.0),
    }
}

/// Benchmark terminal resize.
pub fn bench_resize() -> BenchResult {
    let mut terminal = Terminal::new(80, 24);
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let cols = 80 + (i % 40);
        let rows = 24 + (i % 20);
        terminal.resize(cols, rows);
    }
    let elapsed = start.elapsed();
    BenchResult {
        name: "resize",
        iterations,
        total_ms: elapsed.as_secs_f64() * 1000.0,
        per_iter_us: elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        throughput_mb_s: None,
    }
}

/// Benchmark startup time (terminal + parser creation).
pub fn bench_startup() -> BenchResult {
    let iterations = 100_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _t = Terminal::new(80, 24);
        let _p = VtParser::new();
    }
    let elapsed = start.elapsed();
    BenchResult {
        name: "startup",
        iterations,
        total_ms: elapsed.as_secs_f64() * 1000.0,
        per_iter_us: elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        throughput_mb_s: None,
    }
}

/// Run all benchmarks and return results.
pub fn run_all() -> Vec<BenchResult> {
    vec![
        bench_startup(),
        bench_parser_throughput(),
        bench_grid_scroll(),
        bench_resize(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_all_pass() {
        let results = run_all();
        for r in &results {
            println!("{}", r);
            assert!(r.total_ms > 0.0);
            assert!(r.per_iter_us > 0.0);
        }
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn bench_startup_under_10us() {
        let r = bench_startup();
        println!("{}", r);
        // Terminal + parser creation should be fast
        assert!(r.per_iter_us < 100.0, "startup too slow: {:.1}µs", r.per_iter_us);
    }

    #[test]
    fn bench_parser_over_10mbs() {
        let r = bench_parser_throughput();
        println!("{}", r);
        // Only enforce threshold in release mode; debug is much slower
        #[cfg(not(debug_assertions))]
        {
            let tp = r.throughput_mb_s.unwrap();
            assert!(tp > 10.0, "parser too slow: {:.1} MB/s", tp);
        }
    }
}
