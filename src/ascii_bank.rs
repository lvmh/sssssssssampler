// ─── ASCII Bank ───────────────────────────────────────────────────────────────
//
// Preprocesses all 19 ASCII images at compile time into a normalised grid of
// density indices.  The charset is ordered from blank (0) to dense (max).
//
// Uses a hybrid charset: original ASCII for artwork fidelity, then block
// elements and box drawing chars (all confirmed present in FiraCode Nerd Font)
// for fine density gradation beyond standard ASCII.
//
// IMPORTANT: All original ASCII characters in artwork (letters, punctuation)
// map to themselves via exact-match in char_to_idx — they are NEVER replaced.

/// Ordered charset: index 0 = lightest, index N-1 = densest.
/// Structure:
///   [0..83]   = original ASCII chars (artwork-safe, exact match preserved)
///   [84..N]   = block elements + box drawing for fine density (all in FiraCode)
pub const CHARSET: &[char] = &[
    // ── Original ASCII (indices 0–83) — artwork chars preserved exactly ──
    ' ', '.', '\'', '`', ',', ':', ';', '-', '~', '_',
    '!', 'i', 'l', '1', 'I', 'r', 'c', 'v', 'u', 'n',
    'x', 'z', 'j', 'f', 't', 'L', 'C', 'J', 'Y', 'F',
    'o', 'a', 'e', 's', 'y', 'h', 'k', 'd', 'b', 'p',
    'q', 'g', 'S', 'Z', 'w', 'K', 'U', 'X', 'T', 'H',
    'R', 'E', 'D', 'N', 'V', 'A', 'Q', 'P', 'B', 'G',
    'O', 'M', '0', 'W', '^', '/', '|', '\\', '<', '>',
    '(', ')', '+', '=', '[', ']', '{', '}', '*', '%',
    '#', '&', '$', '@',
    // ── Additional ASCII chars found in source images + UI digits ──
    '"', 'm', '8',
    '2', '3', '4', '5', '6', '7', '9',
    // ── Block elements by visual density ──
    // Light partial blocks (quadrants — sparse coverage)
    '▏', // 84  — 1/8 left block (~12% fill)
    '▎', // 85  — 1/4 left block (~25% fill)
    '▖', // 86  — lower-left quadrant (~25%)
    '▗', // 87  — lower-right quadrant (~25%)
    '▘', // 88  — upper-left quadrant (~25%)
    '▝', // 89  — upper-right quadrant (~25%)
    '▍', // 90  — 3/8 left block (~37%)
    '▚', // 91  — diagonal quadrants (~50%)
    '▞', // 92  — anti-diagonal quadrants (~50%)
    '▌', // 93  — left half (~50%)
    '▐', // 94  — right half (~50%)
    '▄', // 95  — lower half (~50%)
    '▀', // 96  — upper half (~50%)
    '░', // 97  — light shade (~25% stipple)
    '▒', // 98  — medium shade (~50% stipple)
    '▓', // 99  — dark shade (~75% stipple)
    '▙', // 100 — 3-quadrant (~75%)
    '▛', // 101 — 3-quadrant (~75%)
    '▜', // 102 — 3-quadrant (~75%)
    '▟', // 103 — 3-quadrant (~75%)
    '▇', // 104 — 7/8 block (~87%)
    '█', // 105 — full block (100%)
    // ── Box drawing — light to heavy (indices 106+) ──
    '─', // 106 — light horizontal
    '│', // 107 — light vertical
    '┌', // 108 — light corner
    '┐', // 109
    '└', // 110
    '┘', // 111
    '├', // 112
    '┤', // 113
    '┬', // 114
    '┴', // 115
    '┼', // 116 — light cross
    '═', // 117 — double horizontal
    '║', // 118 — double vertical
    '╔', // 119
    '╗', // 120
    '╚', // 121
    '╝', // 122
    '╬', // 123 — double cross (densest box drawing)
];

pub const CHARSET_LEN: usize = CHARSET.len(); // 124

/// Map any char to its nearest index in CHARSET.
/// Called at parse time only.
/// IMPORTANT: Exact ASCII matches always win — artwork characters are preserved.
pub fn char_to_idx(c: char) -> u8 {
    // Fast path: exact match (preserves all original artwork characters)
    for (i, &ch) in CHARSET.iter().enumerate() {
        if ch == c {
            return i as u8;
        }
    }
    // Not in CHARSET — treat as empty space. Don't try to approximate.
    // Source images are plain ASCII; anything unknown is just whitespace.
    0
}

/// A normalised ASCII grid: all cells are indices into CHARSET.
#[derive(Clone)]
pub struct AsciiGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u8>,
}

impl AsciiGrid {
    pub fn get(&self, x: usize, y: usize) -> u8 {
        if x < self.width && y < self.height {
            self.cells[y * self.width + x]
        } else {
            0
        }
    }

    /// Resize to target_w × target_h by nearest-neighbour sampling.
    pub fn resized(&self, target_w: usize, target_h: usize) -> AsciiGrid {
        let mut cells = vec![0u8; target_w * target_h];
        for ty in 0..target_h {
            for tx in 0..target_w {
                let sx = (tx * self.width) / target_w;
                let sy = (ty * self.height) / target_h;
                cells[ty * target_w + tx] = self.get(sx, sy);
            }
        }
        AsciiGrid { width: target_w, height: target_h, cells }
    }
}

#[derive(Clone)]
pub struct AnchorImage {
    pub grid: AsciiGrid,
    /// Fraction of non-space cells (0.0–1.0)
    pub density: f32,
    /// Edge transitions / total cells (0.0–1.0)
    pub complexity: f32,
}

fn compute_density(grid: &AsciiGrid) -> f32 {
    let total = grid.cells.len() as f32;
    if total == 0.0 { return 0.0; }
    let non_space = grid.cells.iter().filter(|&&c| c > 0).count() as f32;
    non_space / total
}

fn compute_complexity(grid: &AsciiGrid) -> f32 {
    let total = grid.cells.len();
    if total == 0 { return 0.0; }
    let mut edges = 0u32;
    for y in 0..grid.height {
        for x in 0..grid.width {
            let c = grid.get(x, y);
            if x + 1 < grid.width && grid.get(x + 1, y) != c { edges += 1; }
            if y + 1 < grid.height && grid.get(x, y + 1) != c { edges += 1; }
        }
    }
    (edges as f32 / total as f32).min(1.0)
}

#[derive(Clone)]
pub struct AsciiBank {
    pub images: Vec<AnchorImage>,
    pub width: usize,
    pub height: usize,
}

impl AsciiBank {
    pub fn from_raw_images(raw: &[&str], target_w: usize, target_h: usize) -> Self {
        let images: Vec<AnchorImage> = raw
            .iter()
            .map(|src| {
                let grid = parse_ascii_image(src, target_w, target_h);
                let density = compute_density(&grid);
                let complexity = compute_complexity(&grid);
                AnchorImage { grid, density, complexity }
            })
            .collect();
        AsciiBank { images, width: target_w, height: target_h }
    }

    /// Parse a single text file with `#N` separators between images.
    pub fn from_ascii_txt(txt: &str) -> Self {
        let mut images = Vec::new();
        let mut current_lines: Vec<&str> = Vec::new();
        let mut in_image = false;

        for line in txt.lines() {
            let trimmed = line.trim();
            // Detect separator: line starts with # followed by a digit, and is short
            if trimmed.starts_with('#') && trimmed.len() < 6 {
                let after_hash = &trimmed[1..];
                if after_hash.chars().all(|c| c.is_ascii_digit()) && !after_hash.is_empty() {
                    // Save previous image if any
                    if in_image && !current_lines.is_empty() {
                        let src = current_lines.join("\n");
                        let grid = parse_ascii_image(&src, 0, 0);
                        let density = compute_density(&grid);
                        let complexity = compute_complexity(&grid);
                        images.push(AnchorImage { grid, density, complexity });
                    }
                    current_lines.clear();
                    in_image = true;
                    continue;
                }
            }
            if in_image {
                current_lines.push(line);
            }
        }
        // Last image
        if in_image && !current_lines.is_empty() {
            let src = current_lines.join("\n");
            let grid = parse_ascii_image(&src, 0, 0);
            let density = compute_density(&grid);
            let complexity = compute_complexity(&grid);
            images.push(AnchorImage { grid, density, complexity });
        }

        let width = images.iter().map(|i| i.grid.width).max().unwrap_or(46);
        let height = images.iter().map(|i| i.grid.height).max().unwrap_or(36);
        AsciiBank { images, width, height }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn get_cell(&self, img_idx: usize, x: usize, y: usize) -> u8 {
        if img_idx < self.images.len() {
            self.images[img_idx].grid.get(x, y)
        } else {
            0
        }
    }
}

fn parse_ascii_image(src: &str, _target_w: usize, _target_h: usize) -> AsciiGrid {
    // Store at native resolution — no resizing, no distortion.
    // Characters render exactly as they appear in the source file.
    let lines: Vec<Vec<u8>> = src
        .lines()
        .map(|line| {
            line.chars()
                .map(|c| char_to_idx(c))
                .collect()
        })
        .collect();

    let raw_h = lines.len().max(1);
    let raw_w = lines.iter().map(|l| l.len()).max().unwrap_or(1).max(1);

    let mut raw_cells = vec![0u8; raw_w * raw_h];
    for (y, line) in lines.iter().enumerate() {
        for (x, &v) in line.iter().enumerate() {
            raw_cells[y * raw_w + x] = v;
        }
    }
    AsciiGrid { width: raw_w, height: raw_h, cells: raw_cells }
}
