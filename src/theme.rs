pub struct Palette {
    pub bg: &'static str,
    pub fg: &'static str,
    pub muted: &'static str,
    pub grid: &'static str,
    pub lanes: [&'static str; 8],
}

pub const LIGHT: Palette = Palette {
    bg: "#ffffff",
    fg: "#0f172a",
    muted: "#64748b",
    grid: "#e2e8f0",
    lanes: [
        "#e11d48", "#ea580c", "#16a34a", "#0891b2", "#2563eb", "#7c3aed", "#db2777", "#0d9488",
    ],
};

pub const DARK: Palette = Palette {
    bg: "#0d1117",
    fg: "#e6edf3",
    muted: "#8b949e",
    grid: "#21262d",
    lanes: [
        "#fb7185", "#fbbf24", "#4ade80", "#22d3ee", "#60a5fa", "#a78bfa", "#f472b6", "#2dd4bf",
    ],
};

impl Palette {
    pub fn vars(&self) -> String {
        let mut s = format!(
            "--bg:{};--fg:{};--muted:{};--grid:{}",
            self.bg, self.fg, self.muted, self.grid
        );
        for (i, c) in self.lanes.iter().enumerate() {
            s.push_str(&format!(";--l{i}:{c}"));
        }
        s
    }
}
