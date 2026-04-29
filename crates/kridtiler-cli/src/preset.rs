/// Built-in semantic presets. Each maps to (cols, rows, x1, y1, x2, y2).
/// Coordinates are 0-indexed and inclusive on both ends.
///
/// The grid is chosen per preset (not a global default) so that, e.g., `thirds-left`
/// can use a 3-col grid without forcing the user to pass `--cols 3`. When --cols/--rows
/// are explicitly passed, they override the preset's grid (useful for `maximize` /
/// `center` which work at any resolution).
pub struct Preset {
    pub cols: u32,
    pub rows: u32,
    pub rect: [u32; 4],
}

pub fn lookup(name: &str) -> Option<Preset> {
    Some(match name {
        // Halves (2x1 / 1x2)
        "half-left"   | "left"   => Preset { cols: 2, rows: 1, rect: [0, 0, 0, 0] },
        "half-right"  | "right"  => Preset { cols: 2, rows: 1, rect: [1, 0, 1, 0] },
        "half-top"    | "top"    => Preset { cols: 1, rows: 2, rect: [0, 0, 0, 0] },
        "half-bottom" | "bottom" => Preset { cols: 1, rows: 2, rect: [0, 1, 0, 1] },

        // Full screen via grid (1x1 — equivalent to clientArea(MaximizeArea))
        "maximize" | "max" | "full" => Preset { cols: 1, rows: 1, rect: [0, 0, 0, 0] },

        // Center: 4x4 inner 2x2 — gives a roughly 50%-area center block.
        "center" => Preset { cols: 4, rows: 4, rect: [1, 1, 2, 2] },

        // Quadrants (2x2)
        "top-left"     | "tl" => Preset { cols: 2, rows: 2, rect: [0, 0, 0, 0] },
        "top-right"    | "tr" => Preset { cols: 2, rows: 2, rect: [1, 0, 1, 0] },
        "bottom-left"  | "bl" => Preset { cols: 2, rows: 2, rect: [0, 1, 0, 1] },
        "bottom-right" | "br" => Preset { cols: 2, rows: 2, rect: [1, 1, 1, 1] },

        // Thirds (horizontal)
        "thirds-left"   => Preset { cols: 3, rows: 1, rect: [0, 0, 0, 0] },
        "thirds-center" => Preset { cols: 3, rows: 1, rect: [1, 0, 1, 0] },
        "thirds-right"  => Preset { cols: 3, rows: 1, rect: [2, 0, 2, 0] },

        // Two-thirds variants (often more useful than pure thirds)
        "two-thirds-left"  => Preset { cols: 3, rows: 1, rect: [0, 0, 1, 0] },
        "two-thirds-right" => Preset { cols: 3, rows: 1, rect: [1, 0, 2, 0] },

        _ => return None,
    })
}

pub fn names() -> &'static [&'static str] {
    &[
        "half-left", "half-right", "half-top", "half-bottom",
        "maximize", "center",
        "top-left", "top-right", "bottom-left", "bottom-right",
        "thirds-left", "thirds-center", "thirds-right",
        "two-thirds-left", "two-thirds-right",
    ]
}
