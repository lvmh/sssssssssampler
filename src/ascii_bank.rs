// ─── ASCII Bank ───────────────────────────────────────────────────────────────
//
// Preprocesses all 19 ASCII images at compile time into a normalised grid of
// density indices.  The charset is ordered from blank (0) to dense (max).
//
// Density mapping is done by a simple lookup: characters not in the charset
// are mapped to the closest density match based on a lookup table.

/// Ordered charset: index 0 = lightest, index N-1 = densest.
pub const CHARSET: &[char] = &[
    ' ', '.', '\'', '`', ',', ':', ';', '-', '~', '_',
    '!', 'i', 'l', '1', 'I', 'r', 'c', 'v', 'u', 'n',
    'x', 'z', 'j', 'f', 't', 'L', 'C', 'J', 'Y', 'F',
    'o', 'a', 'e', 's', 'y', 'h', 'k', 'd', 'b', 'p',
    'q', 'g', 'S', 'Z', 'w', 'K', 'U', 'X', 'T', 'H',
    'R', 'E', 'D', 'N', 'V', 'A', 'Q', 'P', 'B', 'G',
    'O', 'M', '0', 'W', '^', '/', '|', '\\', '<', '>',
    '(', ')', '+', '=', '[', ']', '{', '}', '*', '%',
    '#', '&', '$', '@',
];

pub const CHARSET_LEN: usize = CHARSET.len(); // 84

/// Map any char to its nearest index in CHARSET.
/// Called at parse time only.
pub fn char_to_idx(c: char) -> u8 {
    // Fast path: exact match
    for (i, &ch) in CHARSET.iter().enumerate() {
        if ch == c {
            return i as u8;
        }
    }
    // Fallback density heuristic: use visual weight groups
    let density = visual_density(c);
    let mut best = 0usize;
    let mut best_dist = f32::MAX;
    for (i, _) in CHARSET.iter().enumerate() {
        let cd = visual_density(CHARSET[i]);
        let d = (density - cd).abs();
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best as u8
}

fn visual_density(c: char) -> f32 {
    match c {
        ' ' | '\t' | '\r' | '\n' => 0.0,
        '.' | '\'' | '`' | ',' => 0.05,
        ':' | ';' | '-' | '~' | '_' => 0.12,
        '!' | 'i' | 'l' | '1' | 'I' | 'r' => 0.20,
        'c' | 'v' | 'u' | 'n' | 'x' | 'z' | 'j' | 'f' | 't' => 0.30,
        'L' | 'C' | 'J' | 'Y' | 'F' | 'o' | 'a' | 'e' | 's' | 'y' => 0.38,
        'h' | 'k' | 'd' | 'b' | 'p' | 'q' | 'g' | 'S' | 'Z' | 'w' => 0.46,
        'K' | 'U' | 'X' | 'T' | 'H' | 'R' | 'E' | 'D' | 'N' | 'V' => 0.55,
        'A' | 'Q' | 'P' | 'B' | 'G' | 'O' | 'M' => 0.63,
        '0' | 'W' | '^' | '/' | '|' | '\\' | '<' | '>' => 0.68,
        '(' | ')' | '+' | '=' | '[' | ']' | '{' | '}' => 0.74,
        '*' | '%' | '#' | '&' | '$' | '@' => 0.90,
        _ => 0.50,
    }
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

pub struct AnchorImage {
    pub grid: AsciiGrid,
}

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
                AnchorImage { grid }
            })
            .collect();
        AsciiBank { images, width: target_w, height: target_h }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn get_cell(&self, img_idx: usize, x: usize, y: usize) -> u8 {
        self.images[img_idx].grid.get(x, y)
    }
}

fn parse_ascii_image(src: &str, target_w: usize, target_h: usize) -> AsciiGrid {
    // Split into lines, keeping internal spaces but stripping trailing newlines
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

    // Build a raw grid (pad short lines with spaces = idx 0)
    let mut raw_cells = vec![0u8; raw_w * raw_h];
    for (y, line) in lines.iter().enumerate() {
        for (x, &v) in line.iter().enumerate() {
            raw_cells[y * raw_w + x] = v;
        }
    }
    let raw_grid = AsciiGrid { width: raw_w, height: raw_h, cells: raw_cells };
    raw_grid.resized(target_w, target_h)
}
