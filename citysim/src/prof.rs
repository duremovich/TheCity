//! Scale S1: a per-call-site wall-time profiler, compiled in only with the
//! `prof` cargo feature (`cargo build --release -p citysim-cli --features
//! prof`). Off (the default), [`timed!`] expands to its expression alone and
//! [`dump`] does nothing, so the shipped build reads no clock (`std::time`
//! stays out of the simulation, per `lib.rs`) and pays nothing.
//!
//! Profiling is a measurement of the host, not of the world: nothing here is
//! read by a system, so a profiled run is the same run.

#[cfg(feature = "prof")]
mod imp {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    /// One timed call site: summed nanoseconds and calls.
    pub struct Site {
        ns: AtomicU64,
        calls: AtomicU64,
        registered: AtomicBool,
    }

    static SITES: Mutex<Vec<(&'static str, &'static Site)>> = Mutex::new(Vec::new());

    impl Site {
        #[allow(clippy::new_without_default)]
        pub const fn new() -> Site {
            Site { ns: AtomicU64::new(0), calls: AtomicU64::new(0), registered: AtomicBool::new(false) }
        }

        pub fn add(&'static self, name: &'static str, d: std::time::Duration) {
            self.ns.fetch_add(d.as_nanos() as u64, Ordering::Relaxed);
            self.calls.fetch_add(1, Ordering::Relaxed);
            if !self.registered.swap(true, Ordering::Relaxed) {
                SITES.lock().unwrap_or_else(|e| e.into_inner()).push((name, self));
            }
        }

        /// A guard that adds its lifetime to this site when dropped (a scope
        /// with early `continue`s or `return`s).
        pub fn guard(&'static self, name: &'static str) -> Guard {
            Guard { site: self, name, start: std::time::Instant::now() }
        }
    }

    pub struct Guard {
        site: &'static Site,
        name: &'static str,
        start: std::time::Instant,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            self.site.add(self.name, self.start.elapsed());
        }
    }

    /// Zero every site (a profile of the run's tail: `run --prof-from DAY`).
    pub fn reset() {
        for (_, s) in SITES.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            s.ns.store(0, Ordering::Relaxed);
            s.calls.store(0, Ordering::Relaxed);
        }
    }

    /// Print every site's seconds, share of the top-level total and calls,
    /// slowest first, to stderr as `PROF` lines. The share's denominator is
    /// the sum of the `tick::` sites (the world's tick, system by system).
    pub fn dump() {
        let sites = SITES.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<(u64, u64, &str)> =
            sites.iter().map(|(n, s)| (s.ns.load(Ordering::Relaxed), s.calls.load(Ordering::Relaxed), *n)).collect();
        let total: u64 = v.iter().filter(|(_, _, n)| n.starts_with("tick::")).map(|(ns, _, _)| ns).sum();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(b.2)));
        eprintln!("PROF total {:.2}s (sum of tick::)", total as f64 / 1e9);
        for (ns, calls, name) in v {
            let share = if total > 0 { ns as f64 * 100.0 / total as f64 } else { 0.0 };
            eprintln!("PROF {:>9.3}s {:5.1}% {:>12} {}", ns as f64 / 1e9, share, calls, name);
        }
    }
}

#[cfg(feature = "prof")]
pub use imp::{dump, reset, Guard, Site};

/// Without the `prof` feature: nothing to print.
#[cfg(not(feature = "prof"))]
pub fn dump() {}

/// Without the `prof` feature: nothing to zero.
#[cfg(not(feature = "prof"))]
pub fn reset() {}

/// `timed!("name", expr)`: `expr`, its wall time summed under `name` when
/// built with the `prof` feature; `expr` alone otherwise.
#[cfg(feature = "prof")]
macro_rules! timed {
    ($name:expr, $e:expr) => {{
        static SITE: $crate::prof::Site = $crate::prof::Site::new();
        let t = std::time::Instant::now();
        let r = $e;
        SITE.add($name, t.elapsed());
        r
    }};
}

#[cfg(not(feature = "prof"))]
macro_rules! timed {
    ($name:expr, $e:expr) => {
        $e
    };
}

/// `let _t = scope!("name");`: the rest of the enclosing scope timed under
/// `name` (with the `prof` feature; a zero-sized nothing otherwise).
#[cfg(feature = "prof")]
macro_rules! scope {
    ($name:expr) => {{
        static SITE: $crate::prof::Site = $crate::prof::Site::new();
        SITE.guard($name)
    }};
}

#[cfg(not(feature = "prof"))]
macro_rules! scope {
    ($name:expr) => {
        $crate::prof::NoGuard
    };
}

/// What [`scope!`] holds without the `prof` feature.
#[cfg(not(feature = "prof"))]
pub struct NoGuard;

pub(crate) use {scope, timed};
