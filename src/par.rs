//! A tiny work-stealing `map` over `0..n` on scoped standard-library threads.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Computes `f(0), f(1), ..., f(n - 1)` on `threads` threads and returns the
/// results in index order. Indices are handed out one at a time, so uneven
/// work balances itself.
pub fn parallel_map<T: Send>(n: usize, threads: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut out = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= n {
                return out;
            }
            out.push((i, f(i)));
        }
    };
    let mut results: Vec<(usize, T)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.max(1)).map(|_| scope.spawn(worker)).collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker thread panicked"))
            .collect()
    });
    results.sort_unstable_by_key(|&(i, _)| i);
    results.into_iter().map(|(_, value)| value).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_order() {
        let squares = super::parallel_map(1000, 7, |i| i * i);
        assert_eq!(squares, (0..1000).map(|i| i * i).collect::<Vec<_>>());
        assert!(super::parallel_map(0, 3, |i| i).is_empty());
    }
}
