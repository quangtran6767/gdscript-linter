//! Owner: Track A (W2, W4). Reporters: pretty, compact, json (W2), github and
//! sarif (W4).

/// The value of `lint --format`. Parsed by the CLI in W0; the reporters are
/// implemented in W2 and W4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pretty,
    Compact,
    Json,
    Github,
    Sarif,
}

impl OutputFormat {
    pub fn from_name(name: &str) -> Option<OutputFormat> {
        match name {
            "pretty" => Some(OutputFormat::Pretty),
            "compact" => Some(OutputFormat::Compact),
            "json" => Some(OutputFormat::Json),
            "github" => Some(OutputFormat::Github),
            "sarif" => Some(OutputFormat::Sarif),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Pretty => "pretty",
            OutputFormat::Compact => "compact",
            OutputFormat::Json => "json",
            OutputFormat::Github => "github",
            OutputFormat::Sarif => "sarif",
        }
    }
}
