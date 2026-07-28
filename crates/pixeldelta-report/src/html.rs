use std::fmt::Write;

use crate::{Category, Cluster, Entry, Report};

/// Renders the report as a single self-contained HTML page.
///
/// The page references no external resource: the images are embedded as
/// base64 data URIs and the styles and behavior are inlined, so one file taken
/// out of a CI artifact opens in a browser on its own.
pub fn html(report: &Report) -> String {
    let summary = report.summary();
    let mut ordered: Vec<&Entry> = report.entries.iter().collect();
    // Changed first, then the other differences, matched last; within a
    // category the largest difference leads.
    ordered.sort_by(|a, b| {
        rank(a.category)
            .cmp(&rank(b.category))
            .then(b.diff_ratio.total_cmp(&a.diff_ratio))
    });

    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>pixeldelta report</title>\n<style>\n");
    out.push_str(STYLE);
    out.push_str("\n</style>\n</head>\n<body>\n");

    write!(
        out,
        concat!(
            "<header class=\"app-header\"><div class=\"wrap\">",
            "<div class=\"brandrow\"><div class=\"mark\">\u{0394}</div>",
            "<div><h1>pixeldelta report</h1><div class=\"sub\">expected/ \u{27f7} actual/</div></div>",
            "<span class=\"verdict-pill\"><span class=\"dot\"></span> {verdict}</span></div>",
        ),
        verdict = if summary.passed { "PASS" } else { "FAIL" },
    )
    .unwrap();

    out.push_str("<div class=\"summary\" id=\"summary\">");
    chip(&mut out, "changed", "c-changed", summary.changed, true);
    chip(&mut out, "added", "c-added", summary.added, true);
    chip(&mut out, "removed", "c-removed", summary.removed, true);
    chip(&mut out, "size", "c-size", summary.size_mismatch, true);
    chip(
        &mut out,
        "tolerated",
        "c-tolerated",
        summary.tolerated,
        true,
    );
    chip(&mut out, "matched", "c-matched", summary.matched, false);
    out.push_str("</div>");

    write!(
        out,
        concat!(
            "<div class=\"params\">",
            "<span><b>threshold</b> {threshold:.2}</span>",
            "<span><b>tolerance</b> {tolerance}</span>",
            "<span><b>antialiasing</b> {aa}</span>",
            "<span><b>layout-shift</b> {shift}</span>",
            "</div></div></header>\n",
        ),
        threshold = report.threshold,
        tolerance = report.tolerance_ratio,
        aa = on_off(report.antialiasing),
        shift = on_off(report.layout_shift),
    )
    .unwrap();

    out.push_str("<main class=\"wrap\"><div class=\"entries\" id=\"entries\">\n");
    for (index, entry) in ordered.iter().enumerate() {
        // The leading entry opens so the viewer is visible without a click.
        write_entry(&mut out, entry, index == 0);
    }
    out.push_str("</div></main>\n");

    out.push_str("<button id=\"theme\">theme</button>\n<script>\n");
    out.push_str(SCRIPT);
    out.push_str("\n</script>\n</body>\n</html>\n");
    out
}

/// Sort rank placing changed first and matched last.
fn rank(category: Category) -> u8 {
    match category {
        Category::Changed => 0,
        Category::SizeMismatch => 1,
        Category::Added => 2,
        Category::Removed => 3,
        Category::Tolerated => 4,
        Category::Matched => 5,
    }
}

fn on_off(flag: bool) -> &'static str {
    if flag {
        "on"
    } else {
        "off"
    }
}

fn chip(out: &mut String, cat: &str, class: &str, count: u32, pressed: bool) {
    write!(
        out,
        "<button class=\"chip {class}\" aria-pressed=\"{pressed}\" data-cat=\"{cat}\">\
         <span class=\"swatch\"></span><span class=\"n\">{count}</span> {label}</button>",
        pressed = pressed,
        label = cat.replace("size", "size mismatch"),
    )
    .unwrap();
}

fn write_entry(out: &mut String, entry: &Entry, open: bool) {
    let cat = data_cat(entry.category);
    write!(
        out,
        "<article class=\"entry {cls}{open}\" data-cat=\"{cat}\"><div class=\"entry-head\">\
         <span class=\"stripe\"></span><span class=\"path\">{path}</span>\
         <span class=\"badge {cls}\">{badge}</span><div class=\"metrics\">",
        cls = data_cat(entry.category),
        open = if open { " open" } else { "" },
        path = path_html(&entry.path),
        badge = badge_label(entry.category),
    )
    .unwrap();
    write_metrics(out, entry);
    out.push_str("</div>");
    out.push_str(CHEVRON);
    out.push_str("</div><div class=\"body\">");
    write_body(out, entry);
    out.push_str("</div></article>\n");
}

fn write_metrics(out: &mut String, entry: &Entry) {
    match entry.category {
        Category::Changed | Category::Tolerated => {
            metric(out, &group(entry.diff_pixels), "diff px");
            metric(out, &format!("{:.2}%", entry.diff_ratio * 100.0), "ratio");
            metric(out, &entry.clusters.len().to_string(), "clusters");
        }
        Category::SizeMismatch => {
            metric(out, &size_text(entry.expected_size), "expected");
            metric(out, &size_text(entry.actual_size), "actual");
        }
        Category::Added => metric(out, "only in actual/", "source"),
        Category::Removed => metric(out, "only in expected/", "source"),
        Category::Matched => metric(out, "0", "diff px"),
    }
}

fn metric(out: &mut String, value: &str, key: &str) {
    write!(
        out,
        "<div class=\"metric\"><div class=\"v\">{}</div><div class=\"k\">{}</div></div>",
        escape(value),
        key,
    )
    .unwrap();
}

fn write_body(out: &mut String, entry: &Entry) {
    match entry.category {
        Category::Changed => write_changed_body(out, entry),
        Category::Tolerated => {
            write_changed_body(out, entry);
            out.push_str(
                "<p class=\"inline-note\">Within the allowed difference ratio, so it does not fail the run.</p>",
            );
        }
        Category::SizeMismatch => {
            out.push_str("<div class=\"grid3\" style=\"grid-template-columns:1fr 1fr\">");
            pane(out, "expected", entry.images.expected.as_deref());
            pane(out, "actual", entry.images.actual.as_deref());
            out.push_str(
                "</div><p class=\"inline-note\">Dimensions differ, so no diff was computed.</p>",
            );
        }
        Category::Added => single(out, "actual", entry.images.actual.as_deref()),
        Category::Removed => single(out, "expected", entry.images.expected.as_deref()),
        Category::Matched => {
            single(out, "expected = actual", entry.images.expected.as_deref());
            out.push_str(
                "<p class=\"inline-note\">Identical within the threshold, so expected and actual are the same image.</p>",
            );
        }
    }
}

fn write_changed_body(out: &mut String, entry: &Entry) {
    out.push_str(
        "<div class=\"viewer-bar\"><div class=\"seg\" role=\"tablist\">\
         <button aria-pressed=\"true\" data-mode=\"diff\">Diff</button>\
         <button aria-pressed=\"false\" data-mode=\"side\">Side by side</button>\
         <button aria-pressed=\"false\" data-mode=\"slider\">Slider</button>\
         <button aria-pressed=\"false\" data-mode=\"onion\">Onion</button></div>\
         <div class=\"onion-ctl\">expected<input type=\"range\" min=\"0\" max=\"100\" value=\"50\" class=\"op-range\">actual</div></div>",
    );

    out.push_str("<div class=\"stage\" data-mode=\"diff\"><div class=\"pane m-diff\"><span class=\"cap\">diff</span>");
    img(out, entry.images.diff.as_deref(), "diff");
    write_cluster_rects(out, entry);
    out.push_str("</div><div class=\"grid3 m-side\">");
    pane(out, "expected", entry.images.expected.as_deref());
    pane(out, "actual", entry.images.actual.as_deref());
    out.push_str("</div><div class=\"pane stack m-overlay\"><div class=\"base\">");
    img(out, entry.images.expected.as_deref(), "expected");
    out.push_str("</div><div class=\"layer top\">");
    img(out, entry.images.actual.as_deref(), "actual");
    out.push_str("</div><div class=\"handle\"></div></div></div>");

    write_clusters(out, &entry.clusters);
}

fn write_cluster_rects(out: &mut String, entry: &Entry) {
    let Some([w, h]) = entry.image_size else {
        return;
    };
    let (w, h) = (w as f64, h as f64);
    for c in &entry.clusters {
        write!(
            out,
            "<div class=\"cluster-rect\" style=\"left:{:.3}%;top:{:.3}%;width:{:.3}%;height:{:.3}%\"></div>",
            c.x as f64 / w * 100.0,
            c.y as f64 / h * 100.0,
            c.width as f64 / w * 100.0,
            c.height as f64 / h * 100.0,
        )
        .unwrap();
    }
}

fn write_clusters(out: &mut String, clusters: &[Cluster]) {
    if clusters.is_empty() {
        return;
    }
    out.push_str(
        "<div class=\"clusters\"><h3>Clusters</h3><table><thead><tr>\
         <th>#</th><th>Bounds</th><th>Diff px</th><th>Classification</th><th>SSIM</th>\
         </tr></thead><tbody>",
    );
    for (i, c) in clusters.iter().enumerate() {
        let (label, tag) = classify(c);
        write!(
            out,
            "<tr><td>{n}</td><td>x{x} y{y} \u{b7} {w}\u{d7}{h}</td><td>{px}</td>\
             <td><span class=\"tag {tag}\">{label}</span></td><td>{ssim}</td></tr>",
            n = i + 1,
            x = c.x,
            y = c.y,
            w = c.width,
            h = c.height,
            px = group(c.diff_pixels),
            tag = tag,
            label = label,
            ssim = ssim_cell(c.ssim),
        )
        .unwrap();
    }
    out.push_str("</tbody></table></div>");
}

/// Labels a cluster as a move or a content change from its displacement.
fn classify(c: &Cluster) -> (String, &'static str) {
    match c.displacement {
        Some([dx, dy]) if (dx, dy) != (0, 0) => (moved_label(dx, dy), "moved"),
        _ => ("color change".to_owned(), "color"),
    }
}

fn moved_label(dx: i32, dy: i32) -> String {
    if dx == 0 {
        format!(
            "moved {}px {}",
            dy.abs(),
            if dy > 0 { "down" } else { "up" }
        )
    } else if dy == 0 {
        format!(
            "moved {}px {}",
            dx.abs(),
            if dx > 0 { "right" } else { "left" }
        )
    } else {
        format!("moved {dx},{dy}px")
    }
}

fn ssim_cell(ssim: Option<f64>) -> String {
    match ssim {
        None => "\u{2014}".to_owned(),
        Some(value) => {
            let pct = (value.clamp(0.0, 1.0) * 100.0).round();
            let color = if value >= 0.7 {
                "var(--matched)"
            } else {
                "var(--changed)"
            };
            format!(
                "<span class=\"ssim-bar\"><span class=\"track\">\
                 <span class=\"fill\" style=\"width:{pct}%;background:{color}\"></span></span>{value:.3}</span>",
            )
        }
    }
}

fn single(out: &mut String, cap: &str, png: Option<&[u8]>) {
    out.push_str("<div class=\"single\">");
    pane(out, cap, png);
    out.push_str("</div>");
}

fn pane(out: &mut String, cap: &str, png: Option<&[u8]>) {
    write!(
        out,
        "<div class=\"pane\"><span class=\"cap\">{}</span>",
        escape(cap)
    )
    .unwrap();
    img(out, png, cap);
    out.push_str("</div>");
}

fn img(out: &mut String, png: Option<&[u8]>, alt: &str) {
    if let Some(bytes) = png {
        write!(
            out,
            "<img src=\"data:image/png;base64,{}\" alt=\"{}\">",
            base64(bytes),
            escape(alt),
        )
        .unwrap();
    }
}

fn data_cat(category: Category) -> &'static str {
    match category {
        Category::Changed => "changed",
        Category::SizeMismatch => "size",
        Category::Added => "added",
        Category::Removed => "removed",
        Category::Tolerated => "tolerated",
        Category::Matched => "matched",
    }
}

fn badge_label(category: Category) -> &'static str {
    match category {
        Category::Changed => "changed",
        Category::SizeMismatch => "size mismatch",
        Category::Added => "added",
        Category::Removed => "removed",
        Category::Tolerated => "tolerated",
        Category::Matched => "matched",
    }
}

fn size_text(size: Option<[u32; 2]>) -> String {
    match size {
        Some([w, h]) => format!("{w}\u{d7}{h}"),
        None => "\u{2014}".to_owned(),
    }
}

/// Splits a path into a dimmed directory prefix and the file name.
fn path_html(path: &str) -> String {
    match path.rfind('/') {
        Some(slash) => format!(
            "<span class=\"dir\">{}</span>{}",
            escape(&path[..=slash]),
            escape(&path[slash + 1..]),
        ),
        None => escape(path),
    }
}

/// Groups an integer into thousands with commas.
fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let first = digits.len() % 3;
    for (i, ch) in digits.chars().enumerate() {
        if i != 0 && i >= first && (i - first).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Escapes the HTML text and attribute metacharacters.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Standard base64 of the bytes.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

const CHEVRON: &str = "<svg class=\"chev\" width=\"16\" height=\"16\" viewBox=\"0 0 16 16\" fill=\"none\"><path d=\"M6 4l4 4-4 4\" stroke=\"currentColor\" stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>";

const STYLE: &str = include_str!("report.css");
const SCRIPT: &str = include_str!("report.js");
