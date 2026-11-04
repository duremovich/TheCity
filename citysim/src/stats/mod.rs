//! Daily statistics: counters that systems bump during the day and
//! snapshots taken at the end of it. `--report` prints one row per day.
//!
//! Each column group is one [`cols!`] declaration in its own file (`day.rs`
//! holds the city-wide columns and the order of the groups); the macro makes
//! the struct, its header names and its CSV cells from that one list, so a
//! new counter is one line in its group plus the site that bumps it.

/// One column group: the struct, with `header_into`/`csv_into` (and the
/// joined `header()`/`csv()`) generated from its field list, in field order.
///
/// A field is a column named after itself, printed with `Display`, unless it
/// carries a spec after `=>`:
/// - `fmt("{:.3}")`: the format (`col("name")`, `col("name", "{:.3}")` rename it);
/// - `skip`: not a column (probes, parity counters, the inputs of a computed column);
/// - `group`: a nested group's columns, in place (`opt`: an `Option` group, its
///   columns only when `Some`; its header is appended by `systems::stats::csv_header`);
/// - `with(|h| .., |s, v, districts| ..)`: hand-written header names and cells, for
///   slot groups, arrays and computed columns (a computed column rides on a field it
///   reads; `districts` is the row's district slots).
///
/// Headers are the base slots ([`CORP_SLOTS`], [`DISTRICT_SLOTS`]);
/// [`csv_header`] grows the slot groups for a bigger run.
macro_rules! cols {
    (
        $(#[$sattr:meta])*
        pub struct $name:ident {
            $(
                $(#[$fattr:meta])*
                pub $field:ident : $ty:ty $(=> $kind:ident $(($($arg:tt)*))?)?
            ),* $(,)?
        }
    ) => {
        $(#[$sattr])*
        pub struct $name {
            $( $(#[$fattr])* pub $field: $ty, )*
        }

        impl $name {
            /// The header's names, in column order (the base slots).
            #[allow(unused_variables)]
            pub fn header_into(h: &mut Vec<String>) {
                $( col_header!(h, $field, $ty $(, $kind $(($($arg)*))?)?); )*
            }

            /// The cells, in header order; `districts` is the row's district slots.
            #[allow(unused_variables)]
            pub fn csv_into(&self, v: &mut Vec<String>, districts: usize) {
                $( col_csv!(self, v, districts, $field, $ty $(, $kind $(($($arg)*))?)?); )*
            }

            /// The header's names, comma-separated, in column order.
            pub fn header() -> String {
                let mut h = Vec::new();
                Self::header_into(&mut h);
                h.join(",")
            }

            /// The cells, comma-separated, in header order (the base district slots).
            pub fn csv(&self) -> String {
                let mut v = Vec::new();
                self.csv_into(&mut v, $crate::stats::DISTRICT_SLOTS);
                v.join(",")
            }
        }
    };
}

/// [`cols!`]'s header half of one field.
macro_rules! col_header {
    ($h:ident, $f:ident, $t:ty) => {
        $h.push(stringify!($f).to_string())
    };
    ($h:ident, $f:ident, $t:ty, fmt($fmt:literal)) => {
        $h.push(stringify!($f).to_string())
    };
    ($h:ident, $f:ident, $t:ty, col($n:literal $(, $fmt:literal)?)) => {
        $h.push($n.to_string())
    };
    ($h:ident, $f:ident, $t:ty, skip) => {};
    ($h:ident, $f:ident, $t:ty, opt) => {};
    ($h:ident, $f:ident, $t:ty, group) => {
        <$t>::header_into($h)
    };
    ($h:ident, $f:ident, $t:ty, with($hf:expr, $cf:expr)) => {
        $crate::stats::with_header($h, $hf)
    };
}

/// [`cols!`]'s cell half of one field.
macro_rules! col_csv {
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty) => {
        $v.push($s.$f.to_string())
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, fmt($fmt:literal)) => {
        $v.push(format!($fmt, $s.$f))
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, col($n:literal)) => {
        $v.push($s.$f.to_string())
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, col($n:literal, $fmt:literal)) => {
        $v.push(format!($fmt, $s.$f))
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, skip) => {};
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, opt) => {
        if let Some(g) = &$s.$f {
            g.csv_into($v, $d)
        }
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, group) => {
        $s.$f.csv_into($v, $d)
    };
    ($s:ident, $v:ident, $d:ident, $f:ident, $t:ty, with($hf:expr, $cf:expr)) => {
        $crate::stats::with_csv($s, $v, $d, $cf)
    };
}

/// A `with` field's header closure (typed here, so the closure needs no annotations).
fn with_header(h: &mut Vec<String>, f: impl Fn(&mut Vec<String>)) {
    f(h)
}

/// A `with` field's cell closure.
fn with_csv<S>(s: &S, v: &mut Vec<String>, districts: usize, f: impl Fn(&S, &mut Vec<String>, usize)) {
    f(s, v, districts)
}

mod ambient;
mod budget;
mod captive;
mod contract;
mod day;
mod econ;
mod gov;
mod health;
mod hit;
mod jobs;
mod jobs2;
mod leverage;
mod living;
mod outside;
mod transit;
mod virt;
mod word;
mod wounded;

pub use ambient::*;
pub use budget::*;
pub use captive::*;
pub use contract::*;
pub use day::*;
pub use econ::*;
pub use gov::*;
pub use health::*;
pub use hit::*;
pub use jobs::*;
pub use jobs2::*;
pub use leverage::*;
pub use living::*;
pub use outside::*;
pub use transit::*;
pub use virt::*;
pub use word::*;
pub use wounded::*;

/// Days of history kept for the city panel's sparklines.
pub const STATS_HISTORY_CAP: usize = 120;

/// D38: corp CSV slots (seeding order). M13 D17: 9 (the Tech corp from phase 2).
/// Scale S2: the floor; a config seeding more corps prints one slot per row
/// ([`csv_header`]).
pub const CORP_SLOTS: usize = 9;

/// M15 W43: gang CSV slots (`World::gang_index` order).
pub const GANG_SLOTS: usize = 4;

/// M12 D45: district CSV slots (`[districts]` row order). Scale S2: the
/// floor; a config with more districts prints one slot per district
/// ([`csv_header`]).
pub const DISTRICT_SLOTS: usize = 8;

/// Scale S2: the header for `corps` corp slots and `districts` district
/// slots: the base header ([`DayRow::header`]) itself while both fit its [`CORP_SLOTS`] and
/// [`DISTRICT_SLOTS`] (the 2k city's CSV is unchanged), else every slot
/// group grown past its last slot (`corp10_treasury,corp10_order` after
/// `corp9_order`, `d9_coverage..d9_guards` after `d8_guards`, and so on).
/// The rows match it: a row's slot vectors are as long as the run's slots
/// (`systems::stats::csv_slots`).
pub fn csv_header(corps: usize, districts: usize) -> String {
    let base = DayRow::header();
    if corps <= CORP_SLOTS && districts <= DISTRICT_SLOTS {
        return base;
    }
    // `corp9_order` -> ("corp", 9, "order"); `d8_meals` -> ("d", 8, "meals").
    fn slot(col: &str) -> Option<(&str, usize, &str)> {
        let prefix = ["corp", "c", "d"].into_iter().find(|p| col.starts_with(p))?;
        let rest = &col[prefix.len()..];
        let digits = rest.find('_')?;
        let n = rest[..digits].parse().ok()?;
        Some((prefix, n, &rest[digits + 1..]))
    }
    let cols: Vec<&str> = base.split(',').collect();
    let mut out: Vec<String> = Vec::with_capacity(cols.len());
    let mut block: Vec<&str> = Vec::new();
    for (i, &col) in cols.iter().enumerate() {
        out.push(col.to_string());
        let Some((prefix, n, suffix)) = slot(col) else { continue };
        let (last, want) = if prefix == "d" { (DISTRICT_SLOTS, districts) } else { (CORP_SLOTS, corps) };
        if n != last {
            continue;
        }
        block.push(suffix);
        let next = cols.get(i + 1).and_then(|c| slot(c));
        if next.is_some_and(|(p, m, _)| p == prefix && m == n) {
            continue;
        }
        for k in last + 1..=want {
            out.extend(block.iter().map(|s| format!("{prefix}{k}_{s}")));
        }
        block.clear();
    }
    out.join(",")
}

pub(crate) fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

pub(crate) fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
