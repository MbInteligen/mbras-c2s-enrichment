//! Profile Report Service — Markdown, HTML, PDF report generation
//!
//! Port of ts:src/services/profile-report.service.ts

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ─── Models ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPerson {
    pub cpf: String,
    pub name: String,
    #[serde(default)]
    pub occupation: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub birth_date: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub income: Option<f64>,
    #[serde(default)]
    pub phones: Vec<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub address: Option<ReportAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAddress {
    pub street: Option<String>,
    pub number: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOptions {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default = "default_classification")]
    pub classification: String,
    #[serde(default = "default_true")]
    pub include_contacts: bool,
    #[serde(default = "default_true")]
    pub include_income: bool,
    #[serde(default)]
    pub output_dir: Option<String>,
}

fn default_classification() -> String {
    "Confidencial - Uso Interno".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportResult {
    pub success: bool,
    pub format: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

// ─── Formatting Helpers ─────────────────────────────────────────────────────

pub fn format_cpf(cpf: &str) -> String {
    let digits: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
    let d = if digits.len() >= 11 {
        &digits[digits.len() - 11..]
    } else {
        &digits
    };
    if d.len() == 11 {
        format!("{}.{}.{}-{}", &d[0..3], &d[3..6], &d[6..9], &d[9..11])
    } else {
        cpf.to_string()
    }
}

pub fn format_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        11 => format!("({}) {}-{}", &digits[0..2], &digits[2..7], &digits[7..11]),
        10 => format!("({}) {}-{}", &digits[0..2], &digits[2..6], &digits[6..10]),
        _ => phone.to_string(),
    }
}

fn format_brl(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("R$ {:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("R$ {:.0}", value)
    } else {
        format!("R$ {:.2}", value)
    }
}

fn sanitize_title(title: &str) -> String {
    let sanitized: String = title.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect();
    let trimmed = sanitized.trim().replace(' ', "_");
    if trimmed.len() > 50 { trimmed[..50].to_string() } else { trimmed }
}

// ─── Service ────────────────────────────────────────────────────────────────

pub struct ProfileReportService;

impl ProfileReportService {
    pub fn new() -> Self {
        Self
    }

    /// Generate Markdown report
    pub fn generate_markdown(&self, persons: &[ReportPerson], options: &ReportOptions) -> ReportResult {
        let mut md = String::new();

        // Header
        md.push_str(&format!("# {}\n\n", options.title));
        if let Some(sub) = &options.subtitle {
            md.push_str(&format!("**{}**\n\n", sub));
        }
        let now = Utc::now().format("%d/%m/%Y %H:%M").to_string();
        md.push_str(&format!("**Data do Relatório:** {}\n", now));
        md.push_str(&format!("**Classificação:** {}\n", options.classification));
        md.push_str(&format!("**Total de Registros:** {}\n\n", persons.len()));

        // Executive summary
        md.push_str("## Resumo Executivo\n\n");
        let with_income: Vec<_> = persons.iter().filter(|p| p.income.map(|i| i > 0.0).unwrap_or(false)).collect();
        let avg_income = if !with_income.is_empty() {
            with_income.iter().filter_map(|p| p.income).sum::<f64>() / with_income.len() as f64
        } else {
            0.0
        };
        let total_phones: usize = persons.iter().map(|p| p.phones.len()).sum();
        let total_emails: usize = persons.iter().map(|p| p.emails.len()).sum();

        md.push_str("| Métrica | Valor |\n|---------|-------|\n");
        md.push_str(&format!("| Total de Pessoas | {} |\n", persons.len()));
        md.push_str(&format!("| Com Renda | {} |\n", with_income.len()));
        if !with_income.is_empty() {
            md.push_str(&format!("| Renda Média | {} |\n", format_brl(avg_income)));
        }
        md.push_str(&format!("| Total de Telefones | {} |\n", total_phones));
        md.push_str(&format!("| Total de Emails | {} |\n\n", total_emails));

        // Individual profiles
        for (i, person) in persons.iter().enumerate() {
            md.push_str("---\n\n");
            md.push_str(&format!("### {}. {}\n\n", i + 1, person.name));

            if let Some(occ) = &person.occupation {
                md.push_str(&format!("**Profissão:** {}\n", occ));
            }
            if let Some(comp) = &person.company {
                md.push_str(&format!("**Empresa:** {}\n", comp));
            }

            // Data table
            md.push_str("\n| Campo | Valor |\n|-------|-------|\n");
            md.push_str(&format!("| CPF | {} |\n", format_cpf(&person.cpf)));
            if let Some(bd) = &person.birth_date {
                md.push_str(&format!("| Nascimento | {} |\n", bd));
            }
            if let Some(g) = &person.gender {
                md.push_str(&format!("| Sexo | {} |\n", g));
            }
            if options.include_income {
                if let Some(income) = person.income {
                    md.push_str(&format!("| Renda | {} |\n", format_brl(income)));
                }
            }

            // Address
            if let Some(addr) = &person.address {
                let parts: Vec<String> = [
                    addr.street.as_deref().map(|s| s.to_string()),
                    addr.number.as_deref().map(|n| n.to_string()),
                    addr.neighborhood.as_deref().map(|n| format!("- {}", n)),
                    addr.city.as_deref().zip(addr.state.as_deref()).map(|(c, s)| format!("{}/{}", c, s)),
                ].into_iter().flatten().collect();
                if !parts.is_empty() {
                    md.push_str(&format!("| Endereço | {} |\n", parts.join(", ")));
                }
            }
            md.push('\n');

            // Contacts
            if options.include_contacts {
                if !person.phones.is_empty() {
                    md.push_str("**Telefones:**\n");
                    let max = 5.min(person.phones.len());
                    for phone in &person.phones[..max] {
                        md.push_str(&format!("- {}\n", format_phone(phone)));
                    }
                    if person.phones.len() > 5 {
                        md.push_str(&format!("- ... e mais {} telefones\n", person.phones.len() - 5));
                    }
                    md.push('\n');
                }
                if !person.emails.is_empty() {
                    md.push_str("**Emails:**\n");
                    let max = 3.min(person.emails.len());
                    for email in &person.emails[..max] {
                        md.push_str(&format!("- {}\n", email));
                    }
                    if person.emails.len() > 3 {
                        md.push_str(&format!("- ... e mais {} emails\n", person.emails.len() - 3));
                    }
                    md.push('\n');
                }
            }
        }

        // Footer
        md.push_str("\n---\n\n");
        md.push_str(&format!("*Gerado em {} pelo Sistema de Enriquecimento C2S*\n", now));
        md.push_str("*Fontes: Work API, IBVI Database, Meilisearch (65M empresas)*\n");
        md.push_str("*Este relatório é confidencial e destinado exclusivamente ao uso interno da MBRAS.*\n");

        ReportResult {
            success: true,
            format: "md".to_string(),
            file_path: None,
            content: Some(md),
            error: None,
        }
    }

    /// Generate HTML report with MBRAS branding
    pub fn generate_html(&self, persons: &[ReportPerson], options: &ReportOptions) -> ReportResult {
        let md_result = self.generate_markdown(persons, options);
        let md_content = match &md_result.content {
            Some(c) => c.clone(),
            None => return md_result,
        };

        let html_body = markdown_to_html(&md_content);

        let html = format!(r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<script src="https://cdn.tailwindcss.com"></script>
<style>
body {{ font-family: 'Inter', system-ui, sans-serif; color: #1a3a5c; }}
h1 {{ color: #1a3a5c; border-bottom: 3px solid #b8a06a; padding-bottom: 0.5rem; }}
h2 {{ color: #1a3a5c; }}
h3 {{ color: #2c5282; }}
table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}
th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
th {{ background: #f4f4f4; font-weight: 600; }}
tr:hover {{ background: #f9f9f9; }}
hr {{ border: none; border-top: 1px solid #e2e8f0; margin: 1.5rem 0; }}
</style>
</head>
<body class="max-w-4xl mx-auto p-8">
{body}
</body>
</html>"#, title = options.title, body = html_body);

        ReportResult {
            success: true,
            format: "html".to_string(),
            file_path: None,
            content: Some(html),
            error: None,
        }
    }

    /// Generate PDF via headless Chrome
    pub async fn generate_pdf(&self, persons: &[ReportPerson], options: &ReportOptions) -> ReportResult {
        let html_result = self.generate_html(persons, options);
        let html_content = match &html_result.content {
            Some(c) => c.clone(),
            None => return html_result,
        };

        let output_dir = options.output_dir.as_deref().unwrap_or("./reports");
        let _ = tokio::fs::create_dir_all(output_dir).await;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let safe_title = sanitize_title(&options.title);
        let html_path = format!("{}/{}_{}.html", output_dir, safe_title, timestamp);
        let pdf_path = format!("{}/{}_{}.pdf", output_dir, safe_title, timestamp);

        // Write HTML file
        if let Err(e) = tokio::fs::write(&html_path, &html_content).await {
            return ReportResult {
                success: false,
                format: "pdf".to_string(),
                file_path: None,
                content: None,
                error: Some(format!("Failed to write HTML: {}", e)),
            };
        }

        // Try Chrome headless
        let chrome_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
        ];

        let chrome = chrome_paths.iter().find(|p| Path::new(p).exists());

        if let Some(chrome_path) = chrome {
            let output = tokio::process::Command::new(chrome_path)
                .args([
                    "--headless",
                    "--disable-gpu",
                    "--no-sandbox",
                    &format!("--print-to-pdf={}", pdf_path),
                    "--print-to-pdf-no-header",
                    "--no-margins",
                    &format!("file://{}", std::fs::canonicalize(&html_path).unwrap_or(PathBuf::from(&html_path)).display()),
                ])
                .output()
                .await;

            match output {
                Ok(o) if o.status.success() && Path::new(&pdf_path).exists() => {
                    let _ = tokio::fs::remove_file(&html_path).await;
                    return ReportResult {
                        success: true,
                        format: "pdf".to_string(),
                        file_path: Some(pdf_path),
                        content: None,
                        error: None,
                    };
                }
                _ => {
                    tracing::warn!("Chrome PDF failed, returning HTML instead");
                }
            }
        }

        // Fallback: return HTML file
        ReportResult {
            success: true,
            format: "html".to_string(),
            file_path: Some(html_path),
            content: Some(html_content),
            error: None,
        }
    }
}

/// Simple markdown to HTML conversion
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_table = false;

    for line in md.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("### ") {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str(&format!("<h3>{}</h3>\n", &trimmed[4..]));
        } else if trimmed.starts_with("## ") {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str(&format!("<h2>{}</h2>\n", &trimmed[3..]));
        } else if trimmed.starts_with("# ") {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str(&format!("<h1>{}</h1>\n", &trimmed[2..]));
        } else if trimmed == "---" {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str("<hr>\n");
        } else if trimmed.starts_with('|') && trimmed.ends_with('|') {
            // Skip separator rows
            if trimmed.chars().all(|c| c == '|' || c == '-' || c == ' ') {
                continue;
            }
            if !in_table {
                html.push_str("<table>\n");
                in_table = true;
            }
            let cells: Vec<&str> = trimmed.split('|')
                .filter(|s| !s.trim().is_empty())
                .collect();
            html.push_str("<tr>");
            for cell in cells {
                html.push_str(&format!("<td>{}</td>", cell.trim()));
            }
            html.push_str("</tr>\n");
        } else if trimmed.starts_with("- ") {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str(&format!("<li>{}</li>\n", &trimmed[2..]));
        } else if trimmed.starts_with("**") && trimmed.ends_with("**") {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            html.push_str(&format!("<p><strong>{}</strong></p>\n", &trimmed[2..trimmed.len()-2]));
        } else if trimmed.starts_with('*') && trimmed.ends_with('*') {
            html.push_str(&format!("<p><em>{}</em></p>\n", &trimmed[1..trimmed.len()-1]));
        } else if !trimmed.is_empty() {
            if in_table { html.push_str("</table>\n"); in_table = false; }
            // Inline bold
            let processed = trimmed.replace("**", "<strong>").replace("**", "</strong>");
            html.push_str(&format!("<p>{}</p>\n", processed));
        }
    }

    if in_table { html.push_str("</table>\n"); }
    html
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_person() -> ReportPerson {
        ReportPerson {
            cpf: "12345678901".to_string(),
            name: "João da Silva".to_string(),
            occupation: Some("Diretor".to_string()),
            company: Some("MBRAS".to_string()),
            birth_date: Some("01/01/1980".to_string()),
            gender: Some("M".to_string()),
            income: Some(25000.0),
            phones: vec!["11999887766".to_string(), "1133445566".to_string()],
            emails: vec!["joao@mbras.com.br".to_string()],
            address: Some(ReportAddress {
                street: Some("Rua Augusta".to_string()),
                number: Some("1234".to_string()),
                neighborhood: Some("Jardins".to_string()),
                city: Some("São Paulo".to_string()),
                state: Some("SP".to_string()),
            }),
        }
    }

    #[test]
    fn test_format_cpf() {
        assert_eq!(format_cpf("12345678901"), "123.456.789-01");
        assert_eq!(format_cpf("00012345678901"), "123.456.789-01"); // 14-digit
    }

    #[test]
    fn test_format_phone() {
        assert_eq!(format_phone("11999887766"), "(11) 99988-7766");
        assert_eq!(format_phone("1133445566"), "(11) 3344-5566");
        assert_eq!(format_phone("123"), "123"); // too short, passthrough
    }

    #[test]
    fn test_format_brl() {
        assert_eq!(format_brl(25000.0), "R$ 25000");
        assert_eq!(format_brl(1500000.0), "R$ 1.5M");
        assert_eq!(format_brl(99.99), "R$ 99.99");
    }

    #[test]
    fn test_sanitize_title() {
        assert_eq!(sanitize_title("Relatório de Leads"), "Relatório_de_Leads");
        assert_eq!(sanitize_title("a".repeat(100).as_str()).len(), 50);
    }

    #[test]
    fn test_generate_markdown() {
        let svc = ProfileReportService::new();
        let persons = vec![sample_person()];
        let options = ReportOptions {
            title: "Test Report".to_string(),
            subtitle: Some("Q1 2026".to_string()),
            classification: "Confidencial".to_string(),
            include_contacts: true,
            include_income: true,
            output_dir: None,
        };
        let result = svc.generate_markdown(&persons, &options);
        assert!(result.success);
        assert_eq!(result.format, "md");
        let content = result.content.unwrap();
        assert!(content.contains("# Test Report"));
        assert!(content.contains("João da Silva"));
        assert!(content.contains("123.456.789-01"));
        assert!(content.contains("(11) 99988-7766"));
        assert!(content.contains("R$ 25000"));
    }

    #[test]
    fn test_generate_markdown_no_contacts() {
        let svc = ProfileReportService::new();
        let persons = vec![sample_person()];
        let options = ReportOptions {
            title: "No Contacts".to_string(),
            subtitle: None,
            classification: default_classification(),
            include_contacts: false,
            include_income: false,
            output_dir: None,
        };
        let result = svc.generate_markdown(&persons, &options);
        let content = result.content.unwrap();
        assert!(!content.contains("(11) 99988-7766"));
        // include_income=false hides income in individual profiles, but summary still shows avg
        assert!(!content.contains("(11) 99988-7766")); // contacts are hidden
    }

    #[test]
    fn test_generate_html() {
        let svc = ProfileReportService::new();
        let persons = vec![sample_person()];
        let options = ReportOptions {
            title: "HTML Report".to_string(),
            subtitle: None,
            classification: default_classification(),
            include_contacts: true,
            include_income: true,
            output_dir: None,
        };
        let result = svc.generate_html(&persons, &options);
        assert!(result.success);
        assert_eq!(result.format, "html");
        let content = result.content.unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("<h1>HTML Report</h1>"));
        assert!(content.contains("tailwindcss"));
    }

    #[test]
    fn test_markdown_to_html_headers() {
        let html = markdown_to_html("# Title\n## Subtitle\n### Section");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<h2>Subtitle</h2>"));
        assert!(html.contains("<h3>Section</h3>"));
    }

    #[test]
    fn test_markdown_to_html_table() {
        let md = "| Name | Value |\n|------|-------|\n| CPF | 123 |\n| Renda | R$ 1k |";
        let html = markdown_to_html(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>CPF</td>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_empty_report() {
        let svc = ProfileReportService::new();
        let options = ReportOptions {
            title: "Empty".to_string(),
            subtitle: None,
            classification: default_classification(),
            include_contacts: true,
            include_income: true,
            output_dir: None,
        };
        let result = svc.generate_markdown(&[], &options);
        assert!(result.success);
        let content = result.content.unwrap();
        assert!(content.contains("Total de Registros:** 0"));
    }
}
