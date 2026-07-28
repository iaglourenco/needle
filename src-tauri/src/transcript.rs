use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Preço por milhão de tokens (input, output). Cache write ~1.25x o preço de
/// input (TTL padrão de 5 minutos); cache read ~0.1x o preço de input.
const MODEL_PRICING: &[(&str, f64, f64)] = &[
    ("claude-sonnet-5", 3.00, 15.00),
    ("claude-opus-5", 5.00, 25.00),
    ("claude-haiku-4-5", 1.00, 5.00),
    ("claude-fable-5", 10.00, 50.00),
];

const CACHE_WRITE_MULTIPLIER: f64 = 1.25;
const CACHE_READ_MULTIPLIER: f64 = 0.1;

#[derive(Debug, Clone)]
pub struct SessionUsage {
    pub model: String,
    pub cost_usd: f64,
}

#[derive(Debug, Deserialize)]
struct TranscriptLine {
    #[serde(default)]
    message: Option<TranscriptMessage>,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<TranscriptUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct TranscriptUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

/// Reproduz a sanitização que o Claude Code usa pro nome da pasta de projeto
/// em `~/.claude/projects/`: cada caractere não-alfanumérico vira `-`.
fn sanitize_cwd(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

fn transcript_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(
        home.join(".claude")
            .join("projects")
            .join(sanitize_cwd(cwd))
            .join(format!("{session_id}.jsonl")),
    )
}

fn pricing_for(model: &str) -> Option<(f64, f64)> {
    MODEL_PRICING
        .iter()
        .find(|(name, _, _)| *name == model)
        .map(|(_, input, output)| (*input, *output))
}

fn cost_for(model: &str, usage: &TranscriptUsage) -> f64 {
    let Some((input_price, output_price)) = pricing_for(model) else {
        return 0.0;
    };
    let input_cost = usage.input_tokens as f64 * input_price;
    let output_cost = usage.output_tokens as f64 * output_price;
    let cache_write_cost =
        usage.cache_creation_input_tokens as f64 * input_price * CACHE_WRITE_MULTIPLIER;
    let cache_read_cost =
        usage.cache_read_input_tokens as f64 * input_price * CACHE_READ_MULTIPLIER;
    (input_cost + output_cost + cache_write_cost + cache_read_cost) / 1_000_000.0
}

/// Lê o transcript JSONL da sessão e acumula custo total + modelo mais
/// recente usado. Retorna `None` se o arquivo não existir ou não puder ser
/// lido (sessão muito nova, cwd não bate, etc.) — quem chama trata como
/// "sem dados ainda" em vez de erro.
pub fn read_session_usage(cwd: &str, session_id: &str) -> Option<SessionUsage> {
    let path = transcript_path(cwd, session_id)?;
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut total_cost = 0.0;
    let mut last_model: Option<String> = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<TranscriptLine>(&line) else {
            continue;
        };
        let Some(message) = parsed.message else {
            continue;
        };
        if let Some(model) = &message.model {
            last_model = Some(model.clone());
        }
        if let (Some(model), Some(usage)) = (&message.model, &message.usage) {
            total_cost += cost_for(model, usage);
        }
    }

    let model = last_model?;
    Some(SessionUsage {
        model,
        cost_usd: total_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_cwd_replaces_non_alphanumeric_chars() {
        assert_eq!(
            sanitize_cwd(r"C:\Users\Iago\Documents\Projects\agora"),
            "C--Users-Iago-Documents-Projects-agora"
        );
    }

    #[test]
    fn cost_for_known_model_computes_expected_total() {
        let usage = TranscriptUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert_eq!(cost_for("claude-sonnet-5", &usage), 3.00 + 15.00);
    }

    #[test]
    fn cost_for_unknown_model_is_zero() {
        let usage = TranscriptUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert_eq!(cost_for("some-future-model", &usage), 0.0);
    }

    #[test]
    fn read_session_usage_returns_none_for_missing_file() {
        assert!(read_session_usage("/tmp/does-not-exist", "no-such-session").is_none());
    }
}
