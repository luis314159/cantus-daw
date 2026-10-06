//! The text formats the Linux and macOS readers parse, kept pure so they are tested (and run
//! under Miri) on every platform.

/// A Linux CPU list (`/sys/devices/system/cpu/online`, `shared_cpu_list`): `"0-3,8,10-11"`.
/// Malformed pieces are skipped; the result is sorted and without duplicates.
pub fn cpu_list(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for piece in text
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        match piece.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    if a <= b && b - a < 65_536 {
                        out.extend(a..=b);
                    }
                }
            }
            None => {
                if let Ok(n) = piece.parse::<u32>() {
                    out.push(n);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// A sysfs cache size: `"48K"`, `"24576K"`, `"2M"`, `"1024"` (bytes). `None` if unreadable.
pub fn cache_size(text: &str) -> Option<u64> {
    let t = text.trim();
    let (digits, mult) = match t.as_bytes().last()? {
        b'K' | b'k' => (&t[..t.len() - 1], 1u64 << 10),
        b'M' | b'm' => (&t[..t.len() - 1], 1u64 << 20),
        b'G' | b'g' => (&t[..t.len() - 1], 1u64 << 30),
        _ => (t, 1),
    };
    digits.trim().parse::<u64>().ok()?.checked_mul(mult)
}

/// Ranks distinct capacities (Linux `cpu_capacity`, 1024 = the biggest core) into efficiency
/// classes: the smallest capacity is class 0. Returns each input's class.
pub fn capacity_classes(capacities: &[u32]) -> Vec<u8> {
    let mut distinct: Vec<u32> = capacities.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    capacities
        .iter()
        .map(|c| distinct.iter().position(|d| d == c).unwrap_or(0).min(255) as u8)
        .collect()
}
