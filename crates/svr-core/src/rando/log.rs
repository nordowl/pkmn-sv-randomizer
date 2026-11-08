use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct SpoilerLog {
    pub sections: Vec<LogSection>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LogSection {
    pub title: String,
    pub notes: Vec<String>,
    pub tables: Vec<LogTable>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LogTable {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl SpoilerLog {
    pub fn section(&mut self, title: &str) -> &mut LogSection {
        if let Some(i) = self.sections.iter().position(|s| s.title == title) {
            return &mut self.sections[i];
        }
        self.sections.push(LogSection {
            title: title.to_owned(),
            ..LogSection::default()
        });
        self.sections.last_mut().expect("just pushed")
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for section in &self.sections {
            out.push_str(&format!("== {} ==\n", section.title));
            for note in &section.notes {
                out.push_str(note);
                out.push('\n');
            }
            for table in &section.tables {
                if !table.title.is_empty() {
                    out.push_str(&format!("\n-- {} --\n", table.title));
                }
                let widths: Vec<usize> = (0..table.headers.len())
                    .map(|c| {
                        table
                            .rows
                            .iter()
                            .map(|r| r.get(c).map_or(0, |s| s.chars().count()))
                            .chain([table.headers[c].chars().count()])
                            .max()
                            .unwrap_or(0)
                    })
                    .collect();
                let line = |cells: &[String]| {
                    cells
                        .iter()
                        .zip(&widths)
                        .map(|(cell, &w)| format!("{cell:<w$}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                        .trim_end()
                        .to_owned()
                };
                out.push_str(&line(&table.headers));
                out.push('\n');
                for row in &table.rows {
                    out.push_str(&line(row));
                    out.push('\n');
                }
            }
            out.push('\n');
        }
        out
    }
}

impl LogSection {
    pub fn note(&mut self, text: impl Into<String>) -> &mut Self {
        self.notes.push(text.into());
        self
    }

    pub fn table(&mut self, title: &str, headers: &[&str]) -> &mut LogTable {
        self.tables.push(LogTable {
            title: title.to_owned(),
            headers: headers.iter().map(|h| (*h).to_owned()).collect(),
            rows: Vec::new(),
        });
        self.tables.last_mut().expect("just pushed")
    }
}

impl LogTable {
    pub fn row(&mut self, cells: Vec<String>) -> &mut Self {
        self.rows.push(cells);
        self
    }
}
