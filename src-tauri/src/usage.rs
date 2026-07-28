use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const ANTHROPIC_BETA_HEADER: &str = "oauth-2025-04-20";

// Struct pública enviada ao frontend — camelCase, igual ao resto do app
// (ver settings::Settings). Não deriva Deserialize: a API da Anthropic
// devolve os campos em snake_case (five_hour, seven_day, resets_at), então
// o parse da resposta usa os structs Raw* abaixo e converte pra este.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowUsage {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub five_hour: Option<WindowUsage>,
    pub seven_day: Option<WindowUsage>,
}

// Shape exato da resposta de https://api.anthropic.com/api/oauth/usage
// (snake_case — mesmos nomes que statusline.ps1 lê: $usageData.five_hour.utilization).
#[derive(Debug, Deserialize)]
struct RawWindowUsage {
    #[serde(default)]
    utilization: Option<f64>,
    #[serde(default)]
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAccountUsage {
    #[serde(default)]
    five_hour: Option<RawWindowUsage>,
    #[serde(default)]
    seven_day: Option<RawWindowUsage>,
}

impl From<RawAccountUsage> for AccountUsage {
    fn from(raw: RawAccountUsage) -> Self {
        AccountUsage {
            five_hour: raw.five_hour.and_then(|w| {
                Some(WindowUsage {
                    utilization: w.utilization?,
                    resets_at: w.resets_at,
                })
            }),
            seven_day: raw.seven_day.and_then(|w| {
                Some(WindowUsage {
                    utilization: w.utilization?,
                    resets_at: w.resets_at,
                })
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OauthCreds>,
}

#[derive(Debug, Deserialize)]
struct OauthCreds {
    #[serde(rename = "accessToken")]
    access_token: String,
}

fn credentials_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join(".credentials.json"))
}

fn read_access_token() -> Result<String, String> {
    let path = credentials_path().ok_or("diretório home indisponível")?;
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let creds: CredentialsFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    creds
        .claude_ai_oauth
        .map(|o| o.access_token)
        .ok_or_else(|| "access token OAuth não encontrado em .credentials.json".to_string())
}

pub async fn fetch_account_usage() -> Result<AccountUsage, String> {
    let token = read_access_token()?;

    let client = reqwest::Client::new();
    let response = client
        .get(OAUTH_USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", ANTHROPIC_BETA_HEADER)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("status HTTP {}", response.status()));
    }

    let raw = response
        .json::<RawAccountUsage>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(raw.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_api_snake_case_shape() {
        let body = r#"{
            "five_hour": {"utilization": 42.5, "resets_at": "2026-07-27T18:00:00Z"},
            "seven_day": {"utilization": 12.0, "resets_at": "2026-08-01T00:00:00Z"}
        }"#;
        let raw: RawAccountUsage = serde_json::from_str(body).unwrap();
        let usage: AccountUsage = raw.into();

        let five_hour = usage.five_hour.expect("five_hour deveria estar presente");
        assert_eq!(five_hour.utilization, 42.5);
        assert_eq!(five_hour.resets_at.as_deref(), Some("2026-07-27T18:00:00Z"));

        let seven_day = usage.seven_day.expect("seven_day deveria estar presente");
        assert_eq!(seven_day.utilization, 12.0);
    }

    #[test]
    fn missing_windows_become_none_instead_of_erroring() {
        let raw: RawAccountUsage = serde_json::from_str("{}").unwrap();
        let usage: AccountUsage = raw.into();
        assert!(usage.five_hour.is_none());
        assert!(usage.seven_day.is_none());
    }
}
