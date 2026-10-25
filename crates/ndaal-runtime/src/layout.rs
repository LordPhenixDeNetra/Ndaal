//! Which core serves which load class (ARCHITECTURE.md section 4.1, T02.02).
//! A pure function: no thread, no side effect.

use ndaal_common::config::{CoreCount, CoresConfig};

/// A load class, in priority order (also the tie-break order of rounding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Partitions: key-value, transactions, journal application.
    Shard,
    /// Vector search and index building.
    Vector,
    /// Object streams.
    Transfer,
    /// Compaction and maintenance.
    Background,
    /// Disk operations without `io_uring`.
    Io,
}

const CLASSES: [Class; 5] = [
    Class::Shard,
    Class::Vector,
    Class::Transfer,
    Class::Background,
    Class::Io,
];

/// Rows of the table of ARCHITECTURE.md section 4.1: cores, then
/// (shard, vector, transfer, background, io).
const ROWS: [(usize, [usize; 5]); 5] = [
    (2, [1, 0, 0, 0, 1]),
    (4, [2, 1, 0, 0, 1]),
    (8, [3, 3, 1, 0, 1]),
    (16, [6, 6, 2, 1, 1]),
    (64, [20, 32, 6, 4, 2]),
];

/// Where a class without a core of its own runs: the first class of its
/// chain that has cores. Reproduces the table rows 2, 4 and 8.
fn fallback(class: Class) -> &'static [Class] {
    match class {
        Class::Shard => &[],
        Class::Vector => &[Class::Shard],
        Class::Transfer => &[Class::Vector, Class::Shard],
        Class::Background => &[Class::Transfer, Class::Vector, Class::Shard],
        Class::Io => &[Class::Shard],
    }
}

/// One core: its CPU, its class, and the classes sharing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpec {
    /// CPU the core thread runs on (pinned when allowed).
    pub cpu: usize,
    /// Class the core is dedicated to.
    pub primary: Class,
    /// Classes without a core of their own that run here, in class order.
    pub shared: Vec<Class>,
}

/// Every core of the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Cores in class order, then CPU order.
    pub cores: Vec<CoreSpec>,
}

impl Layout {
    /// Cores dedicated to `class`.
    pub fn count(&self, class: Class) -> usize {
        self.cores.iter().filter(|c| c.primary == class).count()
    }
}

/// A configuration that cannot be laid out.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// More cores requested than allowed to the process.
    #[error("{key} asks for {requested} cores, {available} are available")]
    TooManyCores {
        /// The configuration key.
        key: &'static str,
        /// Cores asked.
        requested: usize,
        /// Cores available.
        available: usize,
    },
    /// No core left for class `shard`, which owns the data.
    #[error("runtime.cores.shard is 0: at least one shard core is needed")]
    NoShard,
    /// No CPU to run on.
    #[error("no CPU available to the process")]
    NoCpu,
}

/// Lays out the cores. `available`: the CPUs allowed to the process, in
/// order; cores take them in class order.
pub fn assign(available: &[usize], config: &CoresConfig) -> Result<Layout, LayoutError> {
    let total = match config.total {
        CoreCount::Auto => available.len(),
        CoreCount::Fixed(n) => n as usize,
    };
    if total > available.len() {
        return Err(LayoutError::TooManyCores {
            key: "runtime.cores.total",
            requested: total,
            available: available.len(),
        });
    }
    if total == 0 {
        return Err(LayoutError::NoCpu);
    }
    let counts = counts(total, config)?;
    if counts[0] == 0 {
        return Err(LayoutError::NoShard);
    }
    let mut cores = Vec::new();
    let mut cpus = available.iter().copied();
    for (class, &count) in CLASSES.iter().zip(&counts) {
        for cpu in cpus.by_ref().take(count) {
            cores.push(CoreSpec {
                cpu,
                primary: *class,
                shared: Vec::new(),
            });
        }
    }
    for (class, &count) in CLASSES.iter().zip(&counts) {
        if count > 0 {
            continue;
        }
        let host = fallback(*class)
            .iter()
            .find(|h| counts[**h as usize] > 0)
            .copied()
            .unwrap_or(Class::Shard);
        if let Some(core) = cores.iter_mut().find(|c| c.primary == host) {
            core.shared.push(*class);
        }
    }
    Ok(Layout { cores })
}

/// Dedicated cores per class for `total` cores: the table, then the values
/// set by the configuration, the rest spread over the `auto` classes.
fn counts(total: usize, config: &CoresConfig) -> Result<[usize; 5], LayoutError> {
    let fixed = [
        config.shard,
        config.vector,
        config.transfer,
        config.background,
        config.io,
    ];
    let table = table(total);
    let fixed_sum: usize = fixed
        .iter()
        .map(|c| match c {
            CoreCount::Fixed(n) => *n as usize,
            CoreCount::Auto => 0,
        })
        .sum();
    if fixed_sum > total {
        return Err(LayoutError::TooManyCores {
            key: "runtime.cores",
            requested: fixed_sum,
            available: total,
        });
    }
    let mut counts = [0; 5];
    let mut weights = [0u64; 5];
    for (i, count) in fixed.iter().enumerate() {
        match count {
            CoreCount::Fixed(n) => counts[i] = *n as usize,
            CoreCount::Auto => weights[i] = table[i] as u64,
        }
    }
    if fixed.iter().all(|c| *c == CoreCount::Auto) {
        return Ok(table);
    }
    let rest = total - fixed_sum;
    if weights.iter().sum::<u64>() == 0 {
        // Auto classes have no core in the table row: the rest goes to the
        // first auto class, shard first.
        if let Some(i) = fixed.iter().position(|c| *c == CoreCount::Auto) {
            counts[i] = rest;
        }
        return Ok(counts);
    }
    let spread = largest_remainder(rest as u64, weights);
    for i in 0..5 {
        counts[i] += spread[i];
    }
    Ok(counts)
}

/// The table row for `total` cores: exact rows, linear interpolation
/// between neighbours, proportions of the 64 row beyond it.
fn table(total: usize) -> [usize; 5] {
    if total == 1 {
        return [1, 0, 0, 0, 0];
    }
    if let Some((_, row)) = ROWS.iter().find(|(n, _)| *n == total) {
        return *row;
    }
    // Shares of each class times a common denominator, summing to
    // `total * den`: exact integers, so ties are exact.
    let num = match ROWS.windows(2).find(|w| w[0].0 < total && total < w[1].0) {
        Some([(lo_n, lo), (hi_n, hi)]) => {
            let den = (hi_n - lo_n) as u64;
            let step = (total - lo_n) as u64;
            let mut num = [0u64; 5];
            for i in 0..5 {
                num[i] = lo[i] as u64 * den + (hi[i] - lo[i]) as u64 * step;
            }
            num
        }
        _ => ROWS[4].1.map(|c| (c * total) as u64),
    };
    largest_remainder(total as u64, num)
}

/// Splits `total` in proportion to `weights` (not all zero): floors, then
/// the remaining units to the largest remainders, ties in class order.
fn largest_remainder(total: u64, weights: [u64; 5]) -> [usize; 5] {
    let den: u64 = weights.iter().sum();
    let scaled = weights.map(|w| w * total);
    let mut counts = scaled.map(|n| (n / den) as usize);
    let mut order: Vec<usize> = (0..5).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(scaled[i] % den));
    let left = total as usize - counts.iter().sum::<usize>();
    for &i in order.iter().take(left) {
        counts[i] += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto() -> CoresConfig {
        CoresConfig {
            total: CoreCount::Auto,
            shard: CoreCount::Auto,
            vector: CoreCount::Auto,
            transfer: CoreCount::Auto,
            background: CoreCount::Auto,
            io: CoreCount::Auto,
            pin: true,
        }
    }

    fn lay(n: usize, config: &CoresConfig) -> Layout {
        let cpus: Vec<usize> = (0..n).collect();
        assign(&cpus, config).expect("valid layout")
    }

    fn row(layout: &Layout) -> [usize; 5] {
        CLASSES.map(|c| layout.count(c))
    }

    fn shared_on(layout: &Layout, host: Class) -> Vec<Class> {
        layout
            .cores
            .iter()
            .find(|c| c.primary == host)
            .map(|c| c.shared.clone())
            .unwrap_or_default()
    }

    #[test]
    fn table_rows_are_exact() {
        for (n, expected) in ROWS {
            assert_eq!(row(&lay(n, &auto())), expected, "{n} cores");
        }
        use Class::*;
        assert_eq!(
            shared_on(&lay(2, &auto()), Shard),
            vec![Vector, Transfer, Background]
        );
        assert_eq!(
            shared_on(&lay(4, &auto()), Vector),
            vec![Transfer, Background]
        );
        assert_eq!(shared_on(&lay(8, &auto()), Transfer), vec![Background]);
    }

    #[test]
    fn interpolation_examples() {
        let cases = [
            (3, [2, 0, 0, 0, 1]),
            (12, [5, 5, 1, 0, 1]),
            (100, [31, 50, 10, 6, 3]),
            (128, [40, 64, 12, 8, 4]),
        ];
        for (n, expected) in cases {
            assert_eq!(row(&lay(n, &auto())), expected, "{n} cores");
        }
        assert_eq!(
            shared_on(&lay(12, &auto()), Class::Transfer),
            vec![Class::Background]
        );
    }

    #[test]
    fn one_core() {
        use Class::*;
        let layout = lay(1, &auto());
        assert_eq!(row(&layout), [1, 0, 0, 0, 0]);
        assert_eq!(
            layout.cores[0].shared,
            vec![Vector, Transfer, Background, Io]
        );
    }

    #[test]
    fn invariants_1_to_256() {
        for n in 1..=256 {
            let layout = lay(n, &auto());
            assert_eq!(layout.cores.len(), n, "{n}");
            assert!(layout.count(Class::Shard) >= 1, "{n}");
            for class in CLASSES {
                let dedicated = layout.count(class) > 0;
                let hosts = layout
                    .cores
                    .iter()
                    .filter(|c| c.shared.contains(&class))
                    .count();
                assert!(
                    (dedicated && hosts == 0) || (!dedicated && hosts == 1),
                    "{n} cores, {class:?}"
                );
            }
            let mut cpus: Vec<usize> = layout.cores.iter().map(|c| c.cpu).collect();
            cpus.sort_unstable();
            assert_eq!(cpus, (0..n).collect::<Vec<_>>(), "{n}");
        }
    }

    #[test]
    fn overrides() {
        let mut config = auto();
        config.shard = CoreCount::Fixed(10);
        let layout = lay(16, &config);
        assert_eq!(layout.count(Class::Shard), 10);
        assert_eq!(layout.cores.len(), 16);

        config.shard = CoreCount::Fixed(0);
        assert_eq!(assign(&[0, 1, 2, 3], &config), Err(LayoutError::NoShard));

        let mut too_many = auto();
        too_many.shard = CoreCount::Fixed(3);
        too_many.vector = CoreCount::Fixed(3);
        assert!(matches!(
            assign(&[0, 1, 2, 3], &too_many),
            Err(LayoutError::TooManyCores { .. })
        ));

        let mut total = auto();
        total.total = CoreCount::Fixed(32);
        assert_eq!(
            assign(&(0..8).collect::<Vec<_>>(), &total),
            Err(LayoutError::TooManyCores {
                key: "runtime.cores.total",
                requested: 32,
                available: 8
            })
        );
        assert_eq!(assign(&[], &auto()), Err(LayoutError::NoCpu));
    }
}
