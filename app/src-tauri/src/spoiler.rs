//! The spoiler files written next to each seed: a short text log and one self-contained HTML page.

use std::fmt::Write;

use svr_core::rando::log::SpoilerLog;

use crate::describe::ChangeLine;
use crate::edition::Edition;

pub struct Header<'a> {
    pub edition: Edition,
    pub seed: &'a str,
    pub created: &'a str,
    pub app_version: &'a str,
}

fn change_path(change: &ChangeLine) -> String {
    format!("{} > {} > {}", change.page.title(), change.group, change.label)
}

pub fn text(header: &Header, changes: &[ChangeLine], log: &SpoilerLog) -> String {
    let mut out = format!(
        "SV Randomizer spoiler log\nGame: {}\nSeed: {}\nCreated: {}\nApp version: {}\n\n== Changed settings ==\n",
        header.edition, header.seed, header.created, header.app_version
    );
    if changes.is_empty() {
        out.push_str("Every option is at its default.\n");
    }
    for change in changes {
        let _ = writeln!(out, "{}: {} (default: {})", change_path(change), change.value, change.default);
    }
    out.push('\n');
    out.push_str(&log.to_text());
    out
}

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn table_html(out: &mut String, title: &str, headers: &[String], rows: &[Vec<String>]) {
    out.push_str("<div class=\"table-wrap\"><table>");
    if !title.is_empty() {
        let _ = write!(out, "<caption>{}</caption>", esc(title));
    }
    out.push_str("<thead><tr>");
    for header in headers {
        let _ = write!(out, "<th scope=\"col\">{}</th>", esc(header));
    }
    out.push_str("</tr></thead><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            let _ = write!(out, "<td>{}</td>", esc(cell));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>\n");
}

pub fn html(header: &Header, changes: &[ChangeLine], log: &SpoilerLog) -> String {
    let (accent, accent_soft) = match header.edition {
        Edition::Scarlet => ("#ff6a3d", "rgba(255, 106, 61, 0.14)"),
        Edition::Violet => ("#b07cff", "rgba(176, 124, 255, 0.14)"),
    };
    let title = format!("{} spoiler log, seed {}", header.edition.short_name(), header.seed);

    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>\n:root {{ --accent: {accent}; --accent-soft: {accent_soft}; }}\n{STYLE}</style>\n</head>\n<body>\n",
        esc(&title)
    );
    let _ = write!(
        out,
        "<header>\n<p class=\"eyebrow\">SV Randomizer spoiler log</p>\n<h1>{}</h1>\n<dl class=\"meta\">\
         <div><dt>Seed</dt><dd>{}</dd></div><div><dt>Created</dt><dd>{}</dd></div>\
         <div><dt>App version</dt><dd>{}</dd></div></dl>\n\
         <div class=\"search\"><input id=\"search\" type=\"search\" placeholder=\"Search this log\" \
         aria-label=\"Search this log\" autocomplete=\"off\" spellcheck=\"false\"><span id=\"count\" aria-live=\"polite\"></span></div>\n</header>\n",
        esc(&header.edition.to_string()),
        esc(header.seed),
        esc(header.created),
        esc(header.app_version)
    );

    out.push_str("<div class=\"layout\">\n<nav aria-label=\"Contents\"><h2>Contents</h2><ol>");
    out.push_str("<li><a href=\"#settings\">Changed settings</a></li>");
    for (index, section) in log.sections.iter().enumerate() {
        let _ = write!(out, "<li><a href=\"#s-{index}\">{}</a></li>", esc(&section.title));
    }
    out.push_str("</ol></nav>\n<main>\n");

    out.push_str("<section id=\"settings\"><h2>Changed settings</h2>\n");
    if changes.is_empty() {
        out.push_str("<ul class=\"notes\"><li>Every option is at its default.</li></ul>\n");
    } else {
        let headers = ["Option", "Value", "Default"].map(str::to_owned);
        let rows: Vec<Vec<String>> = changes
            .iter()
            .map(|change| vec![change_path(change), change.value.clone(), change.default.clone()])
            .collect();
        table_html(&mut out, "", &headers, &rows);
    }
    out.push_str("</section>\n");

    for (index, section) in log.sections.iter().enumerate() {
        let _ = writeln!(out, "<section id=\"s-{index}\"><h2>{}</h2>", esc(&section.title));
        if !section.notes.is_empty() {
            out.push_str("<ul class=\"notes\">");
            for note in &section.notes {
                let _ = write!(out, "<li>{}</li>", esc(note));
            }
            out.push_str("</ul>\n");
        }
        for table in &section.tables {
            table_html(&mut out, &table.title, &table.headers, &table.rows);
        }
        out.push_str("</section>\n");
    }
    out.push_str("</main>\n</div>\n<script>\n");
    out.push_str(SCRIPT);
    out.push_str("</script>\n</body>\n</html>\n");
    out
}

const STYLE: &str = r#"* { box-sizing: border-box; }
[hidden] { display: none !important; }
html { color-scheme: dark; }
body { margin: 0; background: #0e0d12; color: #e9e6f0; font: 15px/1.5 "Segoe UI", system-ui, sans-serif; }
header { padding: 28px 24px 20px; border-bottom: 1px solid #25222e; background: linear-gradient(180deg, var(--accent-soft), transparent); }
.eyebrow { margin: 0; color: var(--accent); font-size: 12px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; }
h1 { margin: 4px 0 14px; font-size: 28px; font-weight: 650; }
.meta { display: flex; flex-wrap: wrap; gap: 8px 28px; margin: 0 0 18px; }
.meta div { min-width: 0; }
.meta dt { color: #9b96aa; font-size: 12px; }
.meta dd { margin: 0; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
.search { display: flex; align-items: center; gap: 12px; max-width: 520px; }
#search { flex: 1; min-width: 0; padding: 9px 12px; border: 1px solid #34303f; border-radius: 8px; background: #16141c; color: inherit; font: inherit; }
#search:focus { outline: 2px solid var(--accent); outline-offset: 1px; border-color: transparent; }
#count { color: #9b96aa; font-size: 13px; white-space: nowrap; }
.layout { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 28px; padding: 24px; }
nav { position: sticky; top: 16px; align-self: start; }
nav h2 { margin: 0 0 8px; color: #9b96aa; font-size: 12px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; }
nav ol { margin: 0; padding: 0; list-style: none; }
nav a { display: block; padding: 4px 8px; border-radius: 6px; color: #cfcbd9; text-decoration: none; }
nav a:hover { background: var(--accent-soft); color: #fff; }
section { margin-bottom: 32px; scroll-margin-top: 16px; }
section h2 { margin: 0 0 10px; padding-left: 10px; border-left: 3px solid var(--accent); font-size: 19px; font-weight: 620; }
.notes { margin: 0 0 12px; padding-left: 20px; color: #d6d2df; }
.table-wrap { overflow-x: auto; margin: 0 0 16px; border: 1px solid #25222e; border-radius: 10px; }
table { width: 100%; border-collapse: collapse; font-size: 14px; }
caption { padding: 10px 12px; color: #cfcbd9; font-weight: 600; text-align: left; }
th { position: sticky; top: 0; background: #1a1821; color: #9b96aa; font-weight: 600; text-align: left; }
th, td { padding: 7px 12px; border-top: 1px solid #25222e; vertical-align: top; }
tbody tr:hover { background: #17151d; }
td { font-variant-numeric: tabular-nums; }
@media (max-width: 760px) {
  header { padding: 20px 16px 16px; }
  .layout { grid-template-columns: minmax(0, 1fr); padding: 16px; gap: 16px; }
  nav { position: static; }
}
"#;

const SCRIPT: &str = r#"(() => {
  const input = document.getElementById('search');
  const count = document.getElementById('count');
  const items = Array.from(document.querySelectorAll('main tbody tr, main .notes li'));
  const texts = items.map((item) => item.textContent.toLowerCase());
  input.addEventListener('input', () => {
    const query = input.value.trim().toLowerCase();
    let shown = 0;
    items.forEach((item, index) => {
      const hit = query === '' || texts[index].includes(query);
      item.hidden = !hit;
      if (hit) shown += 1;
    });
    document.querySelectorAll('main .table-wrap').forEach((wrap) => {
      wrap.hidden = query !== '' && !wrap.querySelector('tbody tr:not([hidden])');
    });
    document.querySelectorAll('main section').forEach((section) => {
      section.hidden = query !== '' && !section.querySelector('tbody tr:not([hidden]), .notes li:not([hidden])');
    });
    count.textContent = query === '' ? '' : shown + (shown === 1 ? ' match' : ' matches');
  });
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use svr_core::rando::options::Page;

    #[test]
    fn every_string_is_escaped() {
        let mut log = SpoilerLog::default();
        log.section("Wild <b>").note("a & b").table("T\"", &["<h>"]).row(vec!["<script>x".into()]);
        let header = Header {
            edition: Edition::Violet,
            seed: "<seed>",
            created: "now",
            app_version: "0.1.0",
        };
        let changes = [ChangeLine {
            id: "x".into(),
            page: Page::Wild,
            group: "<g>".into(),
            label: "L".into(),
            value: "'v'".into(),
            default: "d".into(),
        }];
        let page = html(&header, &changes, &log);
        for raw in ["<seed>", "Wild <b>", "a & b", "<h>", "<script>x", "<g>", "'v'"] {
            assert!(!page.contains(raw), "unescaped {raw}");
        }
        assert!(page.contains("&lt;script&gt;x") && page.contains("#b07cff"));
        let text = text(&header, &changes, &log);
        assert!(text.contains("Wild Pokémon > <g> > L: 'v' (default: d)"), "{text}");
    }
}
