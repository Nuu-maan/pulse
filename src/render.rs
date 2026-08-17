use crate::layout::Layout;
use crate::repo::History;
use crate::theme::{DARK, LIGHT};
use chrono::{DateTime, Datelike, Utc};
use std::collections::BTreeSet;
use std::fmt::Write as _;

const FONT: &str =
    "ui-sans-serif,system-ui,-apple-system,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";
const MONO: &str = "ui-monospace,SFMono-Regular,'Cascadia Code',Menlo,Consolas,monospace";
const REVEAL: f64 = 82.0;
const MAX_STEPS: usize = 80;

#[derive(Clone, Copy, PartialEq)]
pub enum Theme {
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ColorBy {
    Lane,
    Author,
}

pub struct Options {
    pub width: f64,
    pub theme: Theme,
    pub duration: f64,
    pub once: bool,
    pub animate: bool,
    pub color_by: ColorBy,
    pub pulse: bool,
    pub title: Option<String>,
    pub header: bool,
    pub labels: bool,
}

struct Geometry {
    w: f64,
    h: f64,
    pad: f64,
    dx: f64,
    dy: f64,
    top: f64,
    graph_bottom: f64,
    band_axis: f64,
    rule_y: f64,
}

pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}

pub fn svg(h: &History, l: &Layout, o: &Options) -> Rendered {
    let n = h.commits.len();
    let pad = 32.0;
    let dy = 26.0;
    let head_h = if o.header { 58.0 } else { 18.0 };
    let top = head_h + if o.labels { 26.0 } else { 14.0 };

    let span = (o.width - pad * 2.0).max(60.0);
    let dx = if n > 1 {
        (span / (n - 1) as f64).clamp(4.0, 34.0)
    } else {
        0.0
    };
    let w = if n > 1 {
        (pad * 2.0 + dx * (n - 1) as f64).max(380.0)
    } else {
        380.0
    };
    let graph_bottom = top + (l.lanes.saturating_sub(1)) as f64 * dy;

    let band = o.pulse && h.stats && h.commits.iter().any(|c| c.ins + c.del > 0);
    let band_half = 23.0;
    let band_axis = graph_bottom + 22.0 + band_half;
    let rule_y = if band {
        band_axis + band_half + 14.0
    } else {
        graph_bottom + 20.0
    };

    let legend_on = o.color_by == ColorBy::Author && n > 0;

    let g = Geometry {
        w,
        h: rule_y + if legend_on { 42.0 } else { 26.0 },
        pad,
        dx,
        dy,
        top,
        graph_bottom,
        band_axis,
        rule_y,
    };

    let steps = n.clamp(1, MAX_STEPS);
    let bucket = |i: usize| -> usize {
        if n <= 1 || steps <= 1 {
            0
        } else {
            ((i as f64) * (steps - 1) as f64 / (n - 1) as f64).round() as usize
        }
    };

    let color = |i: usize| -> usize {
        match o.color_by {
            ColorBy::Lane => l.lane_of[i] % 8,
            ColorBy::Author => hash(&h.commits[i].email.to_lowercase()) % 8,
        }
    };

    let mut node_steps = BTreeSet::new();
    let mut edge_steps = BTreeSet::new();
    let mut fade_steps = BTreeSet::new();

    let x = |i: usize| g.pad + i as f64 * g.dx;
    let y = |lane: usize| g.top + lane as f64 * g.dy;

    let mut edges = String::new();
    for e in &l.edges {
        let c = color(e.child);
        let b = bucket(e.child);
        let (cx, cy) = (x(e.child), y(l.lane_of[e.child]));
        match e.parent {
            Some(p) => {
                let (px, py) = (x(p), y(l.lane_of[p]));
                let d = if (py - cy).abs() < 0.01 {
                    format!("M{},{}H{}", f(px), f(py), f(cx))
                } else {
                    let mx = px + (cx - px) * 0.55;
                    format!(
                        "M{},{}C{},{} {},{} {},{}",
                        f(px),
                        f(py),
                        f(mx),
                        f(py),
                        f(mx),
                        f(cy),
                        f(cx),
                        f(cy)
                    )
                };
                if o.animate {
                    edge_steps.insert(b);
                    let _ = write!(
                        edges,
                        r#"<path class="e c{c} ek{b}" pathLength="1" stroke-dasharray="1" d="{d}"/>"#
                    );
                } else {
                    let _ = write!(edges, r#"<path class="e c{c}" d="{d}"/>"#);
                }
            }
            None => {
                let stub = (g.dx * 1.8).clamp(10.0, 26.0);
                let d = format!("M{},{}H{}", f(cx), f(cy), f(cx - stub));
                if o.animate {
                    fade_steps.insert(b);
                    let _ = write!(edges, r#"<path class="e stub c{c} fk{b}" d="{d}"/>"#);
                } else {
                    let _ = write!(edges, r#"<path class="e stub c{c}" d="{d}"/>"#);
                }
            }
        }
    }

    let cap = churn_cap(h);
    let weight = |i: usize| -> f64 {
        if !band {
            return 0.5;
        }
        let c = &h.commits[i];
        norm((c.ins + c.del) as f64, cap)
    };

    let mut bars = String::new();
    let mut bar_steps = BTreeSet::new();
    if band {
        let bw = (g.dx * 0.5).clamp(1.6, 9.0);
        let _ = write!(
            bars,
            r#"<line class="rule" x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
            f(g.pad - 10.0),
            f(g.band_axis),
            f(g.w - g.pad + 10.0),
            f(g.band_axis)
        );
        for i in 0..n {
            let c = &h.commits[i];
            if c.ins + c.del == 0 {
                continue;
            }
            let b = bucket(i);
            bar_steps.insert(b);
            let bx = x(i) - bw / 2.0;
            for (v, cls, up) in [(c.ins, "up", true), (c.del, "dn", false)] {
                if v == 0 {
                    continue;
                }
                let bh = (norm(v as f64, cap) * band_half).max(1.4);
                let by = if up { g.band_axis - bh } else { g.band_axis };
                let _ = write!(
                    bars,
                    r#"<rect class="bar {cls} bk{b}" x="{}" y="{}" width="{}" height="{}" rx="{}"/>"#,
                    f(bx),
                    f(by),
                    f(bw),
                    f(bh),
                    f((bw / 2.5).min(1.6))
                );
            }
        }
    }

    let base_r = if n > 1 {
        (g.dx * 0.30).clamp(2.4, 4.4)
    } else {
        4.4
    };
    let mut nodes = String::new();
    for i in 0..n {
        let c = color(i);
        let b = bucket(i);
        let merge = h.commits[i].parents.len() > 1;
        let scaled = base_r * (0.78 + 0.5 * weight(i));
        let r = if merge {
            scaled.max(base_r) + 0.8
        } else if i == n - 1 {
            scaled.max(base_r) + 1.0
        } else {
            scaled
        };
        let cls = if merge { "n m" } else { "n" };
        let anim = if o.animate {
            node_steps.insert(b);
            format!(" nk{b}")
        } else {
            String::new()
        };
        let _ = write!(
            nodes,
            r#"<circle class="{cls} c{c}{anim}" cx="{}" cy="{}" r="{}"><title>{} {} ({}){}</title></circle>"#,
            f(x(i)),
            f(y(l.lane_of[i])),
            f(r),
            esc(&h.commits[i].short),
            esc(&truncate(&h.commits[i].summary, 72)),
            esc(&h.commits[i].author),
            match (h.commits[i].ins, h.commits[i].del) {
                (0, 0) => String::new(),
                (a, d) => format!("\n+{} −{}", human(a), human(d)),
            }
        );
    }

    if o.animate && n > 0 {
        let c = color(n - 1);
        let _ = write!(
            nodes,
            r#"<circle class="hp c{c}" cx="{}" cy="{}" r="{}"/>"#,
            f(x(n - 1)),
            f(y(l.lane_of[n - 1])),
            f(base_r + 1.0)
        );
    }

    let mut labels = String::new();
    if o.labels {
        let budget = if g.dx < 9.0 { 4 } else { 10 };
        let mut used = 0;
        let mut last_x = f64::NEG_INFINITY;
        for i in (0..n).rev() {
            if used >= budget {
                break;
            }
            let Some(label) = h.commits[i].refs.first() else {
                continue;
            };
            let text = &label.name;
            let chip = if label.tag { "tg" } else { "tag" };
            let cx = x(i);
            let tw = 6.1 * text.chars().count() as f64 + 12.0;
            if cx + tw / 2.0 > last_x - 6.0 && last_x.is_finite() {
                continue;
            }
            last_x = cx - tw / 2.0;
            used += 1;
            let b = bucket(i);
            let anim = if o.animate {
                fade_steps.insert(b);
                format!(" fk{b}")
            } else {
                String::new()
            };
            let ly = g.top - 25.0;
            let ny = y(l.lane_of[i]);
            let c = color(i);
            let rx = (cx - tw / 2.0).clamp(4.0, (g.w - tw - 4.0).max(4.0));
            let _ = write!(
                labels,
                r#"<g class="lbl{anim}"><line class="lead c{c}" x1="{0}" y1="{1}" x2="{0}" y2="{2}"/><rect class="{chip} c{c}" x="{3}" y="{4}" width="{5}" height="15" rx="4"/><text class="tagt c{c}" x="{6}" y="{7}">{8}</text></g>"#,
                f(cx),
                f(ly + 15.0),
                f(ny - base_r - 1.5),
                f(rx),
                f(ly),
                f(tw),
                f(rx + tw / 2.0),
                f(ly + 10.8),
                esc(text)
            );
        }
    }

    let mut legend = String::new();
    if legend_on {
        let mut seen: Vec<(String, String, usize)> = Vec::new();
        for c in &h.commits {
            let key = c.email.to_lowercase();
            match seen.iter_mut().find(|(k, _, _)| *k == key) {
                Some((_, _, n)) => *n += 1,
                None => seen.push((key, c.author.clone(), 1)),
            }
        }
        seen.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
        let shown = seen.len().min(6);
        let hidden = seen.len() - shown;

        let width_of = |name: &str| 9.0 + 5.6 * name.chars().count() as f64 + 14.0;
        let mut total: f64 = seen[..shown]
            .iter()
            .map(|(_, name, _)| width_of(name))
            .sum();
        let more = if hidden > 0 {
            format!("+{hidden} more")
        } else {
            String::new()
        };
        if hidden > 0 {
            total += 5.6 * more.chars().count() as f64 + 14.0;
        }

        let mut lx = (g.w - g.pad - total).max(g.pad);
        let ly = g.rule_y + 30.0;
        for (key, name, _) in &seen[..shown] {
            let c = hash(key) % 8;
            let _ = write!(
                legend,
                r#"<circle class="key c{c}" cx="{}" cy="{}" r="3.2"/><text class="keyt" x="{}" y="{}">{}</text>"#,
                f(lx + 3.5),
                f(ly - 3.2),
                f(lx + 10.0),
                f(ly),
                esc(name)
            );
            lx += width_of(name);
        }
        if hidden > 0 {
            let _ = write!(
                legend,
                r#"<text class="keyt muted" x="{}" y="{}">{}</text>"#,
                f(lx),
                f(ly),
                esc(&more)
            );
        }
    }

    let mut axis = String::new();
    let _ = write!(
        axis,
        r#"<line class="rule" x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
        f(g.pad - 10.0),
        f(g.rule_y),
        f(g.w - g.pad + 10.0),
        f(g.rule_y)
    );
    let mut taken: Vec<f64> = Vec::new();
    for (i, gap) in quiet_spells(h) {
        let cx = x(i) - g.dx / 2.0;
        if taken.iter().any(|t| (t - cx).abs() < 40.0) {
            continue;
        }
        taken.push(cx);
        let _ = write!(
            axis,
            r#"<line class="gap" x1="{0}" y1="{1}" x2="{0}" y2="{2}"/><text class="axist" x="{0}" y="{3}">{4}</text>"#,
            f(cx),
            f(g.top - 12.0),
            f(g.rule_y),
            f(g.rule_y + 14.0),
            esc(&span_label(gap))
        );
    }

    let mut last_tick = f64::NEG_INFINITY;
    let mut prev: Option<(i32, u32)> = None;
    for i in 0..n {
        let dt = stamp(h.commits[i].time);
        let key = (dt.year(), dt.month());
        let first = prev.is_none();
        let changed = prev != Some(key);
        prev = Some(key);
        if !changed || first {
            continue;
        }
        let cx = x(i);
        if cx - last_tick < 44.0 || taken.iter().any(|t| (t - cx).abs() < 40.0) {
            continue;
        }
        last_tick = cx;
        taken.push(cx);
        let label = if dt.month() == 1 {
            dt.format("%b %Y").to_string()
        } else {
            dt.format("%b").to_string()
        };
        let _ = write!(
            axis,
            r#"<line class="tick" x1="{0}" y1="{1}" x2="{0}" y2="{2}"/><text class="axist" x="{0}" y="{3}">{4}</text>"#,
            f(cx),
            f(g.top - 12.0),
            f(g.rule_y),
            f(g.rule_y + 14.0),
            esc(&label)
        );
    }

    let mut header = String::new();
    if o.header {
        let title = o.title.clone().unwrap_or_else(|| h.name.clone());
        let authors = h.authors();
        let range = match h.span() {
            Some((a, b)) => {
                let (from, to) = (stamp(a), stamp(b));
                if from.date_naive() == to.date_naive() {
                    to.format("%b %-d, %Y").to_string()
                } else {
                    format!(
                        "{} → {}",
                        from.format("%b %-d, %Y"),
                        to.format("%b %-d, %Y")
                    )
                }
            }
            None => String::new(),
        };
        let churn = match h.churn() {
            (0, 0) => String::new(),
            (ins, del) => format!(" · +{} −{}", human(ins), human(del)),
        };
        let sub = format!(
            "{} commit{}{} · {} author{}{} · {}",
            n,
            plural(n),
            if h.truncated { " (latest)" } else { "" },
            authors,
            plural(authors),
            churn,
            range
        );
        let _ = write!(
            header,
            r#"<text class="h1" x="{}" y="26">{}</text><text class="sub" x="{}" y="44">{}</text><text class="branch" x="{}" y="26">{}</text>"#,
            f(g.pad - 4.0),
            esc(&title),
            f(g.pad - 4.0),
            esc(&sub),
            f(g.w - g.pad + 4.0),
            esc(&h.head)
        );
    }

    let playhead = if o.animate {
        format!(
            r#"<rect class="ph" x="0" y="{}" width="1.5" height="{}" rx="0.75"/>"#,
            f(g.top - 16.0),
            f(g.graph_bottom - g.top + 32.0)
        )
    } else {
        String::new()
    };

    let css = stylesheet(
        o,
        &g,
        steps,
        &node_steps,
        &edge_steps,
        &fade_steps,
        &bar_steps,
    );
    let label = format!(
        "Commit history of {} — {} commits",
        o.title.clone().unwrap_or_else(|| h.name.clone()),
        n
    );

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {hh}" width="{w}" height="{hh}" role="img" aria-label="{label}"><title>{label}</title><style>{css}</style><rect class="bg" width="{w}" height="{hh}" rx="8"/>{axis}{playhead}<g class="wave">{bars}</g><g class="edges">{edges}</g><g class="nodes">{nodes}</g>{labels}{legend}{header}</svg>"##,
        w = f(g.w),
        hh = f(g.h),
        label = esc(&label),
    );

    Rendered {
        svg,
        width: g.w,
        height: g.h,
    }
}

fn stylesheet(
    o: &Options,
    g: &Geometry,
    steps: usize,
    nodes: &BTreeSet<usize>,
    edges: &BTreeSet<usize>,
    fades: &BTreeSet<usize>,
    bars: &BTreeSet<usize>,
) -> String {
    let mut s = String::new();

    match o.theme {
        Theme::Light => {
            let _ = write!(s, ":root{{{}}}", LIGHT.vars());
        }
        Theme::Dark => {
            let _ = write!(s, ":root{{{}}}", DARK.vars());
        }
        Theme::Auto => {
            let _ = write!(s, ":root{{{}}}", LIGHT.vars());
            let _ = write!(
                s,
                "@media(prefers-color-scheme:dark){{:root{{{}}}}}",
                DARK.vars()
            );
        }
    }

    let nw = if g.dx > 0.0 {
        (g.dx * 0.11).clamp(1.0, 1.7)
    } else {
        1.6
    };
    let _ = write!(s, ":root{{--nw:{};--mw:{}}}", f(nw), f(nw + 0.7));

    let _ = write!(
        s,
        ".bg{{fill:var(--bg)}}\
         text{{font-family:{FONT};fill:var(--fg)}}\
         .h1{{font-size:15px;font-weight:650;letter-spacing:-.2px}}\
         .sub{{font-size:11px;fill:var(--muted)}}\
         .branch{{font-family:{MONO};font-size:11px;fill:var(--muted);text-anchor:end}}\
         .axist{{font-size:9px;fill:var(--muted);text-anchor:middle}}\
         .key{{fill:var(--c)}}\
         .keyt{{font-size:9.5px;fill:var(--fg)}}\
         .muted{{fill:var(--muted)}}\
         .rule{{stroke:var(--grid);stroke-width:1}}\
         .tick{{stroke:var(--grid);stroke-width:1;stroke-dasharray:2 4}}\
         .gap{{stroke:var(--muted);stroke-width:1;stroke-dasharray:1 5;opacity:.5}}\
         .e{{fill:none;stroke:var(--c);stroke-width:1.9;stroke-linecap:round}}\
         .stub{{stroke-dasharray:2 3;opacity:.35}}\
         .n{{fill:var(--c);stroke:var(--bg);stroke-width:var(--nw)}}\
         .m{{fill:var(--bg);stroke:var(--c);stroke-width:var(--mw)}}\
         .lead{{stroke:var(--c);stroke-width:1;opacity:.3;stroke-dasharray:2 3}}\
         .bar{{transform-box:fill-box}}\
         .up{{fill:var(--add);opacity:.85;transform-origin:center bottom}}\
         .dn{{fill:var(--del);opacity:.85;transform-origin:center top}}\
         .hp{{fill:none;stroke:var(--c);stroke-width:1.6;opacity:0}}\
         .tag{{fill:var(--c);opacity:.14}}\
         .tg{{fill:none;stroke:var(--c);stroke-width:1;opacity:.5}}\
         .tagt{{font-family:{MONO};font-size:9px;fill:var(--c);text-anchor:middle}}"
    );

    for i in 0..8 {
        let _ = write!(s, ".c{i}{{--c:var(--l{i})}}");
    }

    if !o.animate {
        return s;
    }

    let d = o.duration;
    let count = if o.once { "1" } else { "infinite" };
    let _ = write!(
        s,
        ".n,.e,.lbl,.hp,.ph,.bar{{animation-duration:{d}s;animation-timing-function:linear;animation-iteration-count:{count};animation-fill-mode:both}}\
         .n,.hp{{transform-box:fill-box;transform-origin:center}}"
    );

    let at = |b: usize| -> f64 {
        if steps <= 1 {
            0.0
        } else {
            REVEAL * b as f64 / (steps - 1) as f64
        }
    };

    for &b in nodes {
        let p = at(b);
        let a = (p + 1.8).min(99.0);
        let c = (p + 4.0).min(99.5);
        let _ = write!(
            s,
            ".nk{b}{{animation-name:nk{b}}}@keyframes nk{b}{{0%,{}%{{opacity:0;transform:scale(.2)}}{}%{{opacity:1;transform:scale(1.55)}}{}%,100%{{opacity:1;transform:scale(1)}}}}",
            f(p),
            f(a),
            f(c)
        );
    }

    for &b in edges {
        let p = at(b);
        let start = (p - 4.5).max(0.0);
        let _ = write!(s, ".ek{b}{{animation-name:ek{b}}}@keyframes ek{b}{{");
        if start <= 0.0 {
            let _ = write!(s, "0%{{stroke-dashoffset:1}}");
        } else {
            let _ = write!(s, "0%,{}%{{stroke-dashoffset:1}}", f(start));
        }
        let _ = write!(s, "{}%,100%{{stroke-dashoffset:0}}}}", f(p.max(0.6)));
    }

    for &b in bars {
        let p = at(b);
        let a = (p + 2.4).min(99.5);
        let _ = write!(
            s,
            ".bk{b}{{animation-name:bk{b}}}@keyframes bk{b}{{0%,{}%{{opacity:0;transform:scaleY(0)}}{}%,100%{{opacity:.85;transform:scaleY(1)}}}}",
            f(p),
            f(a)
        );
    }

    for &b in fades {
        let p = at(b);
        let a = (p + 3.0).min(99.5);
        let _ = write!(
            s,
            ".fk{b}{{animation-name:fk{b}}}@keyframes fk{b}{{0%,{}%{{opacity:0}}{}%,100%{{opacity:1}}}}",
            f(p),
            f(a)
        );
    }

    let _ = write!(
        s,
        ".hp{{animation-name:hp}}@keyframes hp{{0%,{r}%{{opacity:0;transform:scale(.5)}}{a}%{{opacity:.6;transform:scale(1.4)}}100%{{opacity:0;transform:scale(3.2)}}}}\
         .ph{{fill:var(--fg);opacity:.14;animation-name:ph}}@keyframes ph{{0%{{transform:translateX({x0}px);opacity:0}}4%{{opacity:.14}}{r}%{{transform:translateX({x1}px);opacity:.14}}{b}%,100%{{transform:translateX({x1}px);opacity:0}}}}",
        r = f(REVEAL),
        a = f(REVEAL + 6.0),
        b = f(REVEAL + 8.0),
        x0 = f(g.pad),
        x1 = f(g.w - g.pad)
    );

    let _ = write!(
        s,
        "@media(prefers-reduced-motion:reduce){{.n,.e,.lbl,.bar{{animation:none!important}}.ph,.hp{{display:none}}}}"
    );

    s
}

fn stamp(t: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(t, 0).unwrap_or_default()
}

fn quiet_spells(h: &History) -> Vec<(usize, i64)> {
    if h.commits.len() < 4 {
        return Vec::new();
    }
    let gaps: Vec<i64> = h
        .commits
        .windows(2)
        .map(|w| (w[1].time - w[0].time).max(0))
        .collect();
    let mut sorted = gaps.clone();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2].max(1);
    let floor = (7 * 86_400).max(median * 4);

    let mut out: Vec<(usize, i64)> = gaps
        .iter()
        .enumerate()
        .filter(|(_, g)| **g >= floor)
        .map(|(i, g)| (i + 1, *g))
        .collect();
    out.sort_by_key(|(_, g)| std::cmp::Reverse(*g));
    out.truncate(8);
    out
}

fn span_label(secs: i64) -> String {
    let days = secs / 86_400;
    match days {
        0..=13 => format!("{days}d"),
        14..=59 => format!("{}w", days / 7),
        60..=729 => format!("{}mo", days / 30),
        _ => format!("{}y", days / 365),
    }
}

fn churn_cap(h: &History) -> f64 {
    let mut v: Vec<usize> = h
        .commits
        .iter()
        .flat_map(|c| [c.ins, c.del])
        .filter(|x| *x > 0)
        .collect();
    if v.is_empty() {
        return 1.0;
    }
    v.sort_unstable();
    let i = ((v.len() as f64 * 0.90) as usize).min(v.len() - 1);
    (v[i] as f64).max(1.0)
}

fn norm(v: f64, cap: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    (v / cap).clamp(0.0, 1.0).powf(0.55)
}

fn human(n: usize) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn hash(s: &str) -> usize {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h >> 8) as usize
}

fn f(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if (r - r.trunc()).abs() < f64::EPSILON {
        format!("{}", r.trunc() as i64)
    } else {
        format!("{r}")
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout;
    use crate::repo::Commit;

    fn history(n: usize) -> History {
        let commits = (0..n)
            .map(|i| Commit {
                id: format!("{i:040}"),
                short: format!("{i:07}"),
                parents: if i == 0 {
                    Vec::new()
                } else {
                    vec![format!("{:040}", i - 1)]
                },
                author: "Test Person".into(),
                email: "test@example.com".into(),
                time: 1_760_000_000 + i as i64 * 86_400,
                summary: format!("commit {i} <tagged> & \"quoted\""),
                refs: Vec::new(),
                ins: 10 + i * 3,
                del: i,
            })
            .collect();
        History {
            name: "demo".into(),
            head: "main".into(),
            commits,
            truncated: false,
            stats: true,
        }
    }

    fn options() -> Options {
        Options {
            width: 800.0,
            theme: Theme::Auto,
            duration: 9.0,
            once: false,
            animate: true,
            color_by: ColorBy::Lane,
            pulse: true,
            title: None,
            header: true,
            labels: true,
        }
    }

    fn render(h: &History, o: &Options) -> String {
        svg(h, &layout::compute(&h.commits), o).svg
    }

    #[test]
    fn the_document_is_self_contained() {
        let h = history(12);
        let out = render(&h, &options());

        assert!(out.starts_with("<svg"));
        assert!(out.ends_with("</svg>"));
        assert!(!out.contains("<script"));
        assert!(!out.contains("xlink"));
        assert!(!out.contains("<image"));
        assert_eq!(
            out.matches("http").count(),
            1,
            "the only URL may be the SVG namespace"
        );
    }

    #[test]
    fn text_from_the_repository_is_escaped() {
        let out = render(&history(3), &options());
        assert!(out.contains("&lt;tagged&gt;"));
        assert!(out.contains("&amp;"));
        assert!(!out.contains("<tagged>"));
    }

    #[test]
    fn every_commit_gets_a_dot() {
        let h = history(25);
        let out = render(&h, &options());
        assert_eq!(out.matches("<circle class=\"n").count(), 25);
    }

    #[test]
    fn a_still_frame_carries_no_animation() {
        let mut o = options();
        o.animate = false;
        let out = render(&history(10), &o);
        assert!(!out.contains("@keyframes"));
        assert!(!out.contains("animation"));
    }

    #[test]
    fn an_animated_frame_defines_keyframes() {
        let out = render(&history(10), &options());
        assert!(out.contains("@keyframes"));
        assert!(out.contains("prefers-reduced-motion"));
    }

    #[test]
    fn the_auto_theme_carries_both_palettes() {
        let out = render(&history(4), &options());
        assert!(out.contains("prefers-color-scheme:dark"));

        let mut o = options();
        o.theme = Theme::Dark;
        assert!(!render(&history(4), &o).contains("prefers-color-scheme"));
    }

    #[test]
    fn geometry_is_always_finite() {
        for n in [1usize, 2, 7, 140, 600] {
            let h = history(n);
            let out = svg(&h, &layout::compute(&h.commits), &options());
            assert!(out.width.is_finite() && out.width > 0.0, "width for {n}");
            assert!(out.height.is_finite() && out.height > 0.0, "height for {n}");
            assert!(!out.svg.contains("NaN"), "NaN in output for {n}");
            assert!(
                !out.svg.contains("=\"inf") && !out.svg.contains("infpx"),
                "infinite coordinate in output for {n}"
            );
        }
    }

    #[test]
    fn dropping_the_pulse_drops_the_waveform() {
        let mut o = options();
        o.pulse = false;
        let out = render(&history(10), &o);
        assert!(!out.contains("class=\"bar"));
    }

    #[test]
    fn helpers_format_as_expected() {
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        assert_eq!(human(999), "999");
        assert_eq!(human(1500), "1.5k");
        assert_eq!(human(2_400_000), "2.4M");
        assert_eq!(span_label(86_400 * 9), "9d");
        assert_eq!(span_label(86_400 * 21), "3w");
        assert_eq!(span_label(86_400 * 180), "6mo");
        assert_eq!(span_label(86_400 * 800), "2y");
        assert_eq!(f(12.0), "12");
        assert_eq!(f(12.34), "12.3");
    }

    #[test]
    fn churn_normalisation_stays_in_range() {
        assert_eq!(norm(0.0, 100.0), 0.0);
        assert_eq!(norm(500.0, 100.0), 1.0);
        let mid = norm(50.0, 100.0);
        assert!(mid > 0.0 && mid < 1.0, "got {mid}");
    }
}
