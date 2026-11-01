//! The view milestone V1 (`OPEN-262`): a config file reloaded into a running world at a tick boundary, for tuning
//! by eye (cityview's `R` key, its file watch and `--reload-at`).
//!
//! The file goes through the transform the world's build put its own config through ([`refit`]: scaled to the
//! world's population when the file's differs, as `run --scaled` and `shadow --population` build; the map's
//! districts and El lines; `Config::fit_to_map` on the world's map), so a reload of the file the world was built
//! from changes nothing. The result is diffed against the world's config key by key (both as `toml::Value`); the
//! live keys are swapped in, every build-time key (`Config::BUILD_TIME`) keeps the world's value and is reported
//! as ignored. A file that does not parse (or breaks a bound) is an `Err` and the world keeps its config.

use crate::config::Config;
use crate::world::World;
use toml::Value;

/// One key that differs: its dotted path and both values as TOML (`None`: absent on that side).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

impl Change {
    /// `[exec] walk_tiles_per_tick 21 -> 42` (a long value cut short).
    pub fn show(&self) -> String {
        let v = |s: &Option<String>| match s {
            // a key a save leaves out (its default, or a section switched off)
            None => "(unset)".to_string(),
            Some(s) if s.chars().count() > 24 => format!("{}..", s.chars().take(22).collect::<String>()),
            Some(s) => s.clone(),
        };
        format!("{} {} -> {}", show_path(&self.path), v(&self.old), v(&self.new))
    }
}

/// `exec.walk_tiles_per_tick` as `[exec] walk_tiles_per_tick`, `buildings.home.capacity` as
/// `[buildings.home] capacity`, a top-level array `trades` as `[[trades]]`.
pub fn show_path(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((section, key)) => format!("[{section}] {key}"),
        None => format!("[[{path}]]"),
    }
}

/// What a reload did: the live keys applied, the build-time keys refused.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub live: Vec<Change>,
    pub ignored: Vec<Change>,
}

impl Report {
    /// The changes listed in the line, each list's first this many (the rest counted).
    const LISTED: usize = 6;

    /// The one line a reload logs and the HUD shows:
    /// `reload: N live keys changed ([exec] x a -> b, ...); M build-time keys ignored ([world] y, [ambient] 7 keys)`.
    /// The ignored keys go by section (a section's one key by name, more counted: a world built with a host's
    /// override, e.g. the shadow's Ambient flats, differs from the shipped file in a whole section).
    pub fn line(&self) -> String {
        let list = |s: Vec<String>| {
            if s.is_empty() {
                return String::new();
            }
            let n = s.len();
            let mut s: Vec<String> = s.into_iter().take(Self::LISTED).collect();
            if n > Self::LISTED {
                s.push(format!("+{} more", n - Self::LISTED));
            }
            format!(" ({})", s.join(", "))
        };
        let mut groups: Vec<(String, Vec<&Change>)> = Vec::new();
        for c in &self.ignored {
            let section = c.path.rsplit_once('.').map_or("", |(s, _)| s).to_string();
            match groups.last_mut() {
                Some((s, v)) if *s == section => v.push(c),
                _ => groups.push((section, vec![c])),
            }
        }
        let ignored = groups
            .into_iter()
            .map(|(s, v)| if v.len() == 1 { show_path(&v[0].path) } else { format!("[{s}] {} keys", v.len()) })
            .collect();
        format!(
            "reload: {} live keys changed{}; {} build-time keys ignored{}",
            self.live.len(),
            list(self.live.iter().map(Change::show).collect()),
            self.ignored.len(),
            list(ignored)
        )
    }

    /// Whether a live key under the dotted `prefix` (a section or a key) changed.
    pub fn touched(&self, prefix: &str) -> bool {
        self.live.iter().any(|c| c.path == prefix || c.path.strip_prefix(prefix).is_some_and(|r| r.starts_with('.')))
    }
}

/// A freshly parsed config put through the transform `world`'s build put its own through: scaled to the world's
/// population when the file's differs (`Config::scaled_to`, as `run --scaled` and `shadow --population`), the
/// world's map named, the map's districts and El lines (`Map::apply_to`), fitted to the map (`fit_to_map`). A
/// config already fitted (a world's own, written out) is not fitted again.
pub fn refit(world: &World, mut cfg: Config) -> Config {
    let pop = world.config.world.population;
    if cfg.world.population != pop {
        cfg = cfg.scaled_to(pop);
    }
    cfg.world.map = world.config.world.map.clone();
    world.map.apply_to(&mut cfg);
    cfg.fit_to_map(&world.map);
    cfg
}

fn to_value(cfg: &Config) -> Result<Value, String> {
    Value::try_from(cfg).map_err(|e| format!("config as TOML: {e}"))
}

/// Every key whose value differs between `a` and `b`, by dotted path in key order. Tables are walked (a table on
/// one side only is walked against nothing); arrays and scalars compare whole.
fn walk(path: &str, a: Option<&Value>, b: Option<&Value>, out: &mut Vec<Change>) {
    let table = |v: Option<&Value>| match v {
        Some(Value::Table(t)) => Some(t.clone()),
        _ => None,
    };
    let (ta, tb) = (table(a), table(b));
    let walkable = (ta.is_some() || a.is_none()) && (tb.is_some() || b.is_none()) && (ta.is_some() || tb.is_some());
    if walkable {
        let (ta, tb) = (ta.unwrap_or_default(), tb.unwrap_or_default());
        let keys: std::collections::BTreeSet<&String> = ta.keys().chain(tb.keys()).collect();
        for k in keys {
            let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
            walk(&p, ta.get(k), tb.get(k), out);
        }
    } else if a != b {
        out.push(Change { path: path.to_string(), old: a.map(show_value), new: b.map(show_value) });
    }
}

/// A value as TOML, an `f32` key's float as the `f32` it is (`0.92`, not the widened `0.9200000166893005`).
fn show_value(v: &Value) -> String {
    match v {
        Value::Float(f) if f64::from(*f as f32) == *f => format!("{}", *f as f32),
        v => v.to_string(),
    }
}

/// The keys that differ between two configs (both serialized as a save writes them).
pub fn diff(old: &Config, new: &Config) -> Result<Vec<Change>, String> {
    let mut out = Vec::new();
    walk("", Some(&to_value(old)?), Some(&to_value(new)?), &mut out);
    Ok(out)
}

fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| v.as_table()?.get(k))
}

/// `path` in `v` set to `to` (created through missing tables), or removed when `to` is `None`.
fn set(v: &mut Value, path: &str, to: Option<Value>) {
    let (parents, key) = match path.rsplit_once('.') {
        Some((p, k)) => (p.split('.').collect::<Vec<_>>(), k),
        None => (vec![], path),
    };
    let mut at = v;
    for p in parents {
        let Some(t) = at.as_table_mut() else { return };
        if !t.contains_key(p) {
            if to.is_none() {
                return;
            }
            t.insert(p.to_string(), Value::Table(Default::default()));
        }
        at = t.get_mut(p).expect("inserted above");
    }
    let Some(t) = at.as_table_mut() else { return };
    match to {
        Some(x) => {
            t.insert(key.to_string(), x);
        }
        None => {
            t.remove(key);
        }
    }
}

/// The world's config with `new`'s live keys: `new` with every build-time path set back to `old`'s.
fn merge(old: &Config, new: &Config) -> Result<Config, String> {
    let ov = to_value(old)?;
    let mut nv = to_value(new)?;
    for b in Config::BUILD_TIME {
        set(&mut nv, b, get(&ov, b).cloned());
    }
    let mut cfg: Config = nv.try_into().map_err(|e: toml::de::Error| format!("merged config: {e}"))?;
    // what a save does not write: the machine's assets and the host's threads
    cfg.assets_dir = old.assets_dir.clone();
    cfg.perf.threads = old.perf.threads;
    Ok(cfg)
}

/// The config `text` would give `world` and what it changes (the world untouched).
fn prepare(world: &World, text: &str) -> Result<(Config, Report), String> {
    let parsed = Config::from_toml(text, &world.config.assets_dir)?;
    let new = refit(world, parsed);
    let ignored: Vec<Change> =
        diff(&world.config, &new)?.into_iter().filter(|c| Config::is_build_time(&c.path)).collect();
    let merged = merge(&world.config, &new)?;
    // what changed is what the merge changed (a build-time key in it would be the merge's bug)
    let live = diff(&world.config, &merged)?;
    debug_assert!(live.iter().all(|c| !Config::is_build_time(&c.path)), "a build-time key passed the merge");
    Ok((merged, Report { live, ignored }))
}

/// Reload `text` (a `config.toml`) into `world` between two ticks: the live keys applied, the build-time keys kept
/// and reported, the caches a live key feeds rebuilt (the walkway portals' costs on an `[exec]` or `[levels]`
/// change, the stat policy on `[lod] policy`, the memos always). An `Err` (a parse error, a broken bound, a panic
/// in the transform) leaves the world as it was.
pub fn reload(world: &mut World, text: &str) -> Result<Report, String> {
    let w: &World = world;
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(w, text))).unwrap_or_else(|p| {
        let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
        Err(msg.unwrap_or_else(|| "the transform panicked".into()))
    });
    // a parse sets the trade keys (`trades_de`) from the file's rows: the world keeps its own
    world.config.set_trade_keys();
    let (cfg, report) = out?;
    if report.live.is_empty() {
        return Ok(report);
    }
    world.config = cfg;
    world.drop_memos();
    if report.touched("exec") || report.touched("levels") {
        crate::systems::levels::init_portals(world);
    }
    if report.touched("lod.policy") {
        world.reload_names();
    }
    Ok(report)
}
