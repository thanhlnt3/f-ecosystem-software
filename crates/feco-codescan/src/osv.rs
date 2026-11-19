//! Tra lỗ hổng thư viện qua API công khai OSV.dev (design S1).
//!
//! - `POST {base}/v1/querybatch`, mỗi lô 500 thư viện; mỗi truy vấn chỉ gồm hệ sinh thái, tên và
//!   phiên bản.
//! - Mỗi mã lỗ hổng: `GET {base}/v1/vulns/{id}` (có bộ nhớ đệm) để lấy mức nghiêm trọng từ
//!   `database_specific.severity`, hoặc tính điểm CVSS v3 từ vector.
//! - CRITICAL và HIGH thành phát hiện `high`; MODERATE, LOW và không rõ thành `medium`.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{json, Value};

use crate::{Category, Dependency, Finding, OsvStatus, Report, Severity};

/// Số truy vấn mỗi lô.
pub const BATCH: usize = 500;
pub const DEFAULT_URL: &str = "https://api.osv.dev";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VulnInfo {
    pub id: String,
    pub summary: String,
    /// `CRITICAL`, `HIGH`, `MODERATE`, `LOW`; `None` khi không rõ.
    pub severity: Option<String>,
    pub withdrawn: bool,
}

#[derive(Clone)]
pub struct OsvClient {
    base: String,
    http: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, VulnInfo>>>,
}

impl std::fmt::Debug for OsvClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsvClient")
            .field("base", &self.base)
            .finish()
    }
}

impl OsvClient {
    pub fn new(base: &str) -> Self {
        OsvClient {
            base: base.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .user_agent(concat!("feco-codescan/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("tạo HTTP client"),
            cache: Arc::default(),
        }
    }

    /// Từ biến môi trường: `FECO_OSV=off` thì `None`; địa chỉ từ `FECO_OSV_URL` (mặc định
    /// <https://api.osv.dev>).
    pub fn from_env() -> Option<Self> {
        if std::env::var("FECO_OSV").is_ok_and(|v| v.eq_ignore_ascii_case("off")) {
            return None;
        }
        let base = std::env::var("FECO_OSV_URL").unwrap_or_else(|_| DEFAULT_URL.to_owned());
        Some(OsvClient::new(&base))
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Mã lỗ hổng cho từng thư viện (cùng thứ tự với `deps`).
    pub async fn query(
        &self,
        deps: &[(String, String, String)],
    ) -> Result<Vec<Vec<String>>, String> {
        let mut out = Vec::with_capacity(deps.len());
        for chunk in deps.chunks(BATCH) {
            let queries: Vec<Value> = chunk
                .iter()
                .map(|(ecosystem, name, version)| {
                    json!({"package": {"ecosystem": ecosystem, "name": name}, "version": version})
                })
                .collect();
            let res = self
                .http
                .post(format!("{}/v1/querybatch", self.base))
                .json(&json!({ "queries": queries }))
                .send()
                .await
                .map_err(|e| format!("OSV querybatch: {e}"))?;
            if !res.status().is_success() {
                return Err(format!("OSV querybatch trả HTTP {}", res.status().as_u16()));
            }
            let body: Value = res
                .json()
                .await
                .map_err(|e| format!("OSV querybatch: {e}"))?;
            let results = body["results"]
                .as_array()
                .ok_or("OSV querybatch: thiếu results")?;
            if results.len() != chunk.len() {
                return Err(format!(
                    "OSV querybatch trả {} kết quả cho {} truy vấn",
                    results.len(),
                    chunk.len()
                ));
            }
            for r in results {
                out.push(
                    r["vulns"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v["id"].as_str().map(str::to_owned))
                        .collect(),
                );
            }
        }
        Ok(out)
    }

    /// Chi tiết một lỗ hổng (có bộ nhớ đệm).
    pub async fn vuln(&self, id: &str) -> Result<VulnInfo, String> {
        if let Some(v) = self.cache.lock().unwrap().get(id) {
            return Ok(v.clone());
        }
        let res = self
            .http
            .get(format!("{}/v1/vulns/{id}", self.base))
            .send()
            .await
            .map_err(|e| format!("OSV vulns/{id}: {e}"))?;
        if !res.status().is_success() {
            return Err(format!("OSV vulns/{id} trả HTTP {}", res.status().as_u16()));
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| format!("OSV vulns/{id}: {e}"))?;
        let info = VulnInfo {
            id: id.to_owned(),
            summary: body["summary"]
                .as_str()
                .or_else(|| body["details"].as_str())
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect(),
            severity: severity_of(&body),
            withdrawn: body.get("withdrawn").is_some_and(|w| !w.is_null()),
        };
        self.cache
            .lock()
            .unwrap()
            .insert(id.to_owned(), info.clone());
        Ok(info)
    }
}

fn normalize(s: &str) -> Option<String> {
    match s.trim().to_ascii_uppercase().as_str() {
        "CRITICAL" => Some("CRITICAL".into()),
        "HIGH" => Some("HIGH".into()),
        "MODERATE" | "MEDIUM" => Some("MODERATE".into()),
        "LOW" => Some("LOW".into()),
        _ => None,
    }
}

fn from_score(score: f64) -> String {
    match score {
        s if s >= 9.0 => "CRITICAL",
        s if s >= 7.0 => "HIGH",
        s if s >= 4.0 => "MODERATE",
        _ => "LOW",
    }
    .into()
}

/// Mức nghiêm trọng của một bản ghi OSV.
pub fn severity_of(v: &Value) -> Option<String> {
    if let Some(s) = v["database_specific"]["severity"]
        .as_str()
        .and_then(normalize)
    {
        return Some(s);
    }
    for a in v["affected"].as_array().into_iter().flatten() {
        for key in ["database_specific", "ecosystem_specific"] {
            if let Some(s) = a[key]["severity"].as_str().and_then(normalize) {
                return Some(s);
            }
        }
    }
    v["severity"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["type"].as_str() == Some("CVSS_V3"))
        .filter_map(|s| s["score"].as_str().and_then(cvss3_base))
        .reduce(f64::max)
        .map(from_score)
}

/// Điểm cơ bản CVSS v3.x từ vector (`CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H`).
pub fn cvss3_base(vector: &str) -> Option<f64> {
    let m: BTreeMap<&str, &str> = vector
        .split('/')
        .skip(1)
        .filter_map(|p| p.split_once(':'))
        .collect();
    let changed = match *m.get("S")? {
        "U" => false,
        "C" => true,
        _ => return None,
    };
    let av = match *m.get("AV")? {
        "N" => 0.85,
        "A" => 0.62,
        "L" => 0.55,
        "P" => 0.2,
        _ => return None,
    };
    let ac = match *m.get("AC")? {
        "L" => 0.77,
        "H" => 0.44,
        _ => return None,
    };
    let pr = match (*m.get("PR")?, changed) {
        ("N", _) => 0.85,
        ("L", false) => 0.62,
        ("L", true) => 0.68,
        ("H", false) => 0.27,
        ("H", true) => 0.5,
        _ => return None,
    };
    let ui = match *m.get("UI")? {
        "N" => 0.85,
        "R" => 0.62,
        _ => return None,
    };
    let cia = |k: &str| -> Option<f64> {
        Some(match *m.get(k)? {
            "H" => 0.56,
            "L" => 0.22,
            "N" => 0.0,
            _ => return None,
        })
    };
    let iss = 1.0 - (1.0 - cia("C")?) * (1.0 - cia("I")?) * (1.0 - cia("A")?);
    let impact = if changed {
        7.52 * (iss - 0.029) - 3.25 * (iss - 0.02).powi(15)
    } else {
        6.42 * iss
    };
    if impact <= 0.0 {
        return Some(0.0);
    }
    let exploit = 8.22 * av * ac * pr * ui;
    let raw = if changed {
        (1.08 * (impact + exploit)).min(10.0)
    } else {
        (impact + exploit).min(10.0)
    };
    Some((raw * 10.0 - 1e-9).ceil() / 10.0)
}

/// Tra OSV cho mọi thư viện trong báo cáo; thêm phát hiện `vulnerability.osv` và đặt `osv` thành
/// `checked` hoặc `failed(lý do)`.
pub async fn check_vulnerabilities(report: &mut Report, osv: &OsvClient) {
    let mut unique: Vec<(String, String, String)> = report
        .dependencies
        .iter()
        .map(|d| (d.ecosystem.clone(), d.name.clone(), d.version.clone()))
        .collect();
    unique.sort();
    unique.dedup();
    if unique.is_empty() {
        report.osv = OsvStatus::Checked;
        return;
    }
    let ids = match osv.query(&unique).await {
        Ok(ids) => ids,
        Err(e) => {
            report.osv = OsvStatus::Failed(e);
            return;
        }
    };
    let by_dep: HashMap<&(String, String, String), &Vec<String>> =
        unique.iter().zip(&ids).collect();
    let mut all: Vec<String> = ids.iter().flatten().cloned().collect();
    all.sort();
    all.dedup();
    let mut infos: HashMap<String, VulnInfo> = HashMap::new();
    for chunk in all.chunks(16) {
        let mut set = tokio::task::JoinSet::new();
        for id in chunk {
            let (osv, id) = (osv.clone(), id.clone());
            set.spawn(async move { osv.vuln(&id).await });
        }
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(info)) => {
                    infos.insert(info.id.clone(), info);
                }
                Ok(Err(e)) => {
                    report.osv = OsvStatus::Failed(e);
                    return;
                }
                Err(e) => {
                    report.osv = OsvStatus::Failed(e.to_string());
                    return;
                }
            }
        }
    }
    let deps: Vec<Dependency> = report.dependencies.clone();
    for d in &deps {
        let key = (d.ecosystem.clone(), d.name.clone(), d.version.clone());
        for id in by_dep.get(&key).copied().into_iter().flatten() {
            let Some(info) = infos.get(id) else { continue };
            if info.withdrawn {
                continue;
            }
            let severity = match info.severity.as_deref() {
                Some("CRITICAL" | "HIGH") => Severity::High,
                _ => Severity::Medium,
            };
            let level = info.severity.as_deref().unwrap_or("không rõ mức");
            report.findings.push(Finding {
                rule: "vulnerability.osv".into(),
                category: Category::Vulnerability,
                severity,
                path: d.path.clone(),
                line: d.line,
                message: format!(
                    "{} {} ({}): {} [{level}] {}",
                    d.name, d.version, d.ecosystem, info.id, info.summary
                )
                .trim_end()
                .to_owned(),
                excerpt: None,
            });
        }
    }
    report.osv = OsvStatus::Checked;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvss_scores() {
        assert_eq!(
            cvss3_base("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"),
            Some(9.8)
        );
        assert_eq!(
            cvss3_base("CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N"),
            Some(6.1)
        );
        assert_eq!(
            cvss3_base("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"),
            Some(7.5)
        );
        assert_eq!(
            cvss3_base("CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:N/I:N/A:N"),
            Some(0.0)
        );
        assert_eq!(cvss3_base("CVSS:4.0/AV:N"), None);
    }

    #[test]
    fn severities() {
        assert_eq!(
            severity_of(&json!({"database_specific": {"severity": "MODERATE"}})).as_deref(),
            Some("MODERATE")
        );
        assert_eq!(
            severity_of(&json!({"severity": [{"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"}]})).as_deref(),
            Some("CRITICAL")
        );
        assert_eq!(
            severity_of(&json!({"affected": [{"ecosystem_specific": {"severity": "HIGH"}}]}))
                .as_deref(),
            Some("HIGH")
        );
        assert_eq!(severity_of(&json!({"id": "X"})), None);
    }
}
