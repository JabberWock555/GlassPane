//! Air quality for the home location from Open-Meteo (free, no API key).
//! Uses the system `curl` (present in Termux) so the phone build needs no
//! TLS stack. Refreshes every 20 minutes.

use crate::hub::Hub;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const REFRESH: Duration = Duration::from_secs(20 * 60);
const RETRY: Duration = Duration::from_secs(2 * 60);

pub async fn run(hub: Arc<Hub>, fixed: Option<[f64; 2]>) {
    loop {
        let loc = fixed.or_else(|| {
            hub.location
                .read()
                .unwrap()
                .as_ref()
                .map(|l| [l.lat, l.lon])
        });
        let Some([lat, lon]) = loc else {
            // Waiting for Home Assistant to report its location.
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        };
        let base = std::env::var("GLASSPANE_AQI_URL")
            .unwrap_or_else(|_| "https://air-quality-api.open-meteo.com/v1/air-quality".into());
        let url = format!("{base}?latitude={lat:.4}&longitude={lon:.4}&current=us_aqi,pm2_5,pm10");
        match fetch(&url).await.and_then(|v| parse(&v)) {
            Ok(aqi) => {
                info!("AQI {}", aqi["aqi"]);
                hub.set_aqi(aqi);
                tokio::time::sleep(REFRESH).await;
            }
            Err(e) => {
                warn!("AQI fetch failed: {e:#}");
                tokio::time::sleep(RETRY).await;
            }
        }
    }
}

async fn fetch(url: &str) -> anyhow::Result<Value> {
    let out = tokio::process::Command::new("curl")
        .args(["-sS", "--max-time", "20", url])
        .output()
        .await?;
    if !out.status.success() {
        anyhow::bail!("curl: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

/// Turn an Open-Meteo response into `{aqi, pm25, pm10, category, time}`.
pub fn parse(v: &Value) -> anyhow::Result<Value> {
    let cur = &v["current"];
    let aqi = cur["us_aqi"]
        .as_f64()
        .ok_or_else(|| anyhow::anyhow!("no us_aqi in response: {v}"))?
        .round() as i64;
    Ok(json!({
        "aqi": aqi,
        "pm25": cur["pm2_5"].as_f64().map(|x| (x * 10.0).round() / 10.0),
        "pm10": cur["pm10"].as_f64().map(|x| x.round()),
        "category": category(aqi),
        "time": cur["time"],
        "source": "Open-Meteo (US AQI)",
    }))
}

pub fn category(aqi: i64) -> &'static str {
    match aqi {
        i64::MIN..=50 => "Good",
        51..=100 => "Moderate",
        101..=150 => "Unhealthy for sensitive groups",
        151..=200 => "Unhealthy",
        201..=300 => "Very unhealthy",
        _ => "Hazardous",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_meteo_current() {
        let v = json!({"current": {"time": "2026-11-01T00:45", "interval": 900,
                       "us_aqi": 87.4, "pm2_5": 29.36, "pm10": 51.2}});
        let a = parse(&v).unwrap();
        assert_eq!(a["aqi"], 87);
        assert_eq!(a["category"], "Moderate");
        assert_eq!(a["pm25"], 29.4);
        assert!(parse(&json!({"error": true})).is_err());
    }
}
