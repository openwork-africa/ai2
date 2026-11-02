use crate::harness::Finding;
use crate::scan::Spec;
use crate::step::Item;
use crate::steps::{Exploit, Rating, Technology};
use serde::Serialize;
use std::fmt::Write;

#[derive(Serialize)]
pub struct Review {
    pub version: &'static str,
    pub folder: String,
    pub objective: String,
    pub generated_at: String,
    pub harness: Option<&'static str>,
    pub ratings: Vec<Card>,
    pub technologies: Vec<Technology>,
    pub exploits: Vec<Exploit>,
    pub findings: Vec<Finding>,
    pub spec: Spec,
    pub notes: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct Card {
    pub key: &'static str,
    pub name: &'static str,
    pub short: &'static str,
    pub riskier: bool,
    pub rating: Option<Rating>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
}

pub fn html(review: &Review) -> String {
    let mut page = String::new();
    let _ = write!(page, "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>ai2 security review</title><style>{STYLE}</style></head><body><main>");
    let _ = write!(
        page,
        "<header><h1>Security review</h1><p>{} · {}</p>",
        esc(&review.folder),
        esc(&review.generated_at)
    );
    if !review.objective.is_empty() {
        let _ = write!(
            page,
            "<p class=\"objective\">{}</p>",
            esc(&review.objective)
        );
    }
    page.push_str("</header><section class=\"ratings\">");
    for card in &review.ratings {
        let (name, short) = (card.name, card.short);
        let _ = write!(
            page,
            "<article class=\"rating\"><h2>{name}<span>{short}</span></h2>"
        );
        match &card.rating {
            Some(r) => {
                let _ = write!(
                    page,
                    "<p class=\"score {}\">{}%</p><ul>",
                    tone(r.score, card.riskier),
                    r.score
                );
                for f in &r.factors {
                    let _ = write!(
                        page,
                        "<li><b>{}</b> {}%<br><small>{}</small></li>",
                        esc(&f.name),
                        f.score,
                        esc(&f.why)
                    );
                }
                page.push_str("</ul>");
            }
            None => {
                let _ = write!(
                    page,
                    "<p class=\"score none\">—</p><p class=\"missing\">{}</p>",
                    esc(card.missing.as_deref().unwrap_or_default())
                );
            }
        }
        page.push_str("</article>");
    }
    page.push_str("</section>");
    table(
        &mut page,
        "Technology",
        &["Technology", "Kind", "Share of the risk"],
        review.technologies.iter().map(|t| {
            vec![
                esc(&t.name),
                t.kind.into(),
                format!(
                    "<span class=\"bar\" style=\"--w:{}%\"></span>{}%",
                    t.risk, t.risk
                ),
            ]
        }),
    );
    table(
        &mut page,
        "Recent exploits",
        &["Id", "Technology", "Severity", "Fixed in", "Summary"],
        review.exploits.iter().map(|e| {
            vec![
                esc(&e.id),
                esc(&e.technology),
                esc(&e.severity),
                esc(&e.fixed_in),
                esc(&e.summary),
            ]
        }),
    );
    table(
        &mut page,
        "Findings",
        &["Finding", "Where", "Severity", "Status", "Fix"],
        review.findings.iter().map(|f| {
            vec![
                esc(&f.title),
                esc(&f.file),
                esc(&f.severity),
                esc(&f.status),
                esc(&f.fix),
            ]
        }),
    );
    for card in review.ratings.iter().filter(|c| !c.items.is_empty()) {
        let _ = write!(page, "<section><h2>{}</h2><ol class=\"items\">", card.name);
        for t in &card.items {
            let why = t
                .factors
                .iter()
                .map(|f| format!("{} {}%: {}", f.name, f.score, f.why))
                .collect::<Vec<_>>()
                .join(" · ");
            let _ = write!(
                page,
                "<li><span class=\"item {}\">{} {}%</span><p>{}</p><small>{}</small></li>",
                tone(t.score, card.riskier),
                card.short,
                t.score,
                esc(&t.text),
                esc(&why)
            );
        }
        page.push_str("</ol></section>");
    }
    if !review.notes.is_empty() {
        page.push_str("<section><h2>Notes</h2><ul>");
        for note in &review.notes {
            let _ = write!(page, "<li>{}</li>", esc(note));
        }
        page.push_str("</ul></section>");
    }
    page.push_str("</main></body></html>\n");
    page
}

fn table(page: &mut String, title: &str, heads: &[&str], rows: impl Iterator<Item = Vec<String>>) {
    let rows: Vec<_> = rows.collect();
    let _ = write!(page, "<section><h2>{title}</h2>");
    if rows.is_empty() {
        page.push_str("<p class=\"missing\">None.</p></section>");
        return;
    }
    let _ = write!(
        page,
        "<table><thead><tr>{}</tr></thead><tbody>",
        heads
            .iter()
            .map(|h| format!("<th>{h}</th>"))
            .collect::<String>()
    );
    for row in rows {
        let _ = write!(
            page,
            "<tr>{}</tr>",
            row.iter()
                .zip(heads)
                .map(|(c, h)| format!("<td data-label=\"{h}\">{c}</td>"))
                .collect::<String>()
        );
    }
    page.push_str("</tbody></table></section>");
}

fn tone(score: f64, riskier: bool) -> &'static str {
    let good = if riskier { 100.0 - score } else { score };
    if good >= 80.0 {
        "good"
    } else if good >= 50.0 {
        "fair"
    } else {
        "poor"
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = r#"
:root{--bg:#fafaf9;--card:#fff;--text:#1c1917;--muted:#78716c;--line:#e7e5e4;--good:#15803d;--fair:#b45309;--poor:#b91c1c;--accent:#2563eb}
@media (prefers-color-scheme:dark){:root{--bg:#0c0a09;--card:#1c1917;--text:#f5f5f4;--muted:#a8a29e;--line:#292524;--good:#4ade80;--fair:#fbbf24;--poor:#f87171;--accent:#60a5fa}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,sans-serif}
main{max-width:1080px;margin:0 auto;padding:32px 16px}header p{color:var(--muted);margin:4px 0}.objective{color:var(--text)!important}
h1{margin:0;font-size:28px}h2{font-size:17px;margin:32px 0 12px}
.ratings{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:16px;margin-top:24px}
.rating{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px}.rating h2{margin:0;display:flex;justify-content:space-between}.rating h2 span{color:var(--muted);font-weight:500}
.score{font-size:44px;font-weight:700;margin:8px 0}.good{color:var(--good)}.fair{color:var(--fair)}.poor{color:var(--poor)}.none{color:var(--muted)}
.rating ul{list-style:none;padding:0;margin:0}.rating li{padding:6px 0;border-top:1px solid var(--line)}small,.missing{color:var(--muted)}
header p{overflow-wrap:anywhere}
table{width:100%;border-collapse:separate;border-spacing:0;background:var(--card);border:1px solid var(--line);border-radius:12px;overflow:hidden}
tbody tr:last-child td{border-bottom:0}
@media (max-width:640px){thead{display:none}table,tbody,tr,td{display:block;width:100%}tr{padding:8px 0;border-bottom:1px solid var(--line)}tbody tr:last-child{border-bottom:0}td{border:0;padding:4px 12px;overflow-wrap:anywhere}td::before{content:attr(data-label);display:block;color:var(--muted);font-size:12px}}
th,td{text-align:left;padding:8px 12px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-weight:500}
.bar{display:inline-block;width:80px;height:6px;border-radius:3px;margin-right:8px;background:linear-gradient(90deg,var(--accent) var(--w),var(--line) var(--w))}
.items{padding-left:20px}.items li{margin-bottom:12px}.items p{margin:2px 0}.item{font-weight:600;font-size:13px}
"#;
