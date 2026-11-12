//! Kiểm tra SPF, DKIM, DMARC của tên miền gửi email (spec notifications, "Không lộ thông tin nhạy
//! cảm qua thông báo": email MUST gửi từ tên miền có SPF, DKIM và DMARC).
//!
//! F-Eco gửi email qua một máy chủ chuyển tiếp SMTP có TLS (nhà cung cấp email giao dịch, hoặc
//! Postfix kèm OpenDKIM) ký DKIM cho tên miền; ở đây chỉ kiểm tra bản ghi DNS và gợi ý bản ghi
//! cần thêm. `feco-catalog mail-dns <tên miền> <selector>` chạy kiểm tra này.

use std::net::SocketAddr;

use serde::Serialize;

use crate::dns;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Report {
    pub spf: Option<String>,
    pub dkim: Option<String>,
    pub dmarc: Option<String>,
    /// Vấn đề tìm thấy; rỗng là đạt.
    pub problems: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Đánh giá các bản ghi TXT đã tra được.
pub fn evaluate(root: &[String], dkim: &[String], dmarc: &[String]) -> Report {
    let mut r = Report::default();
    let spf: Vec<&String> = root
        .iter()
        .filter(|t| t.trim_start().to_ascii_lowercase().starts_with("v=spf1"))
        .collect();
    match spf.as_slice() {
        [] => r
            .problems
            .push("thiếu bản ghi SPF (TXT v=spf1 ... ở tên miền)".into()),
        [one] => {
            r.spf = Some((*one).clone());
            let t = one.trim_end().to_ascii_lowercase();
            if !(t.ends_with("-all") || t.ends_with("~all")) {
                r.problems
                    .push("SPF nên kết thúc bằng -all hoặc ~all".into());
            }
        }
        _ => r.problems.push("có nhiều bản ghi SPF; chỉ được một".into()),
    }
    match dkim.iter().find(|t| {
        t.contains("p=") && (t.to_ascii_lowercase().contains("v=dkim1") || !t.contains("v="))
    }) {
        Some(t)
            if t.split(';').any(|p| {
                p.trim()
                    .strip_prefix("p=")
                    .is_some_and(|k| !k.trim().is_empty())
            }) =>
        {
            r.dkim = Some(t.clone())
        }
        Some(t) => {
            r.dkim = Some(t.clone());
            r.problems
                .push("khóa DKIM trống (p= rỗng nghĩa là đã thu hồi)".into());
        }
        None => r
            .problems
            .push("thiếu bản ghi DKIM (TXT v=DKIM1; p=... ở <selector>._domainkey)".into()),
    }
    match dmarc
        .iter()
        .find(|t| t.trim_start().to_ascii_lowercase().starts_with("v=dmarc1"))
    {
        Some(t) => {
            r.dmarc = Some(t.clone());
            let policy = t
                .split(';')
                .filter_map(|p| p.trim().strip_prefix("p="))
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if !matches!(policy.as_str(), "quarantine" | "reject") {
                r.problems.push(format!(
                    "DMARC p={policy}: nên là quarantine hoặc reject khi đã gửi ổn định"
                ));
            }
        }
        None => r
            .problems
            .push("thiếu bản ghi DMARC (TXT v=DMARC1; p=... ở _dmarc)".into()),
    }
    r
}

/// Tra bản ghi qua DNS rồi đánh giá.
pub async fn check(
    servers: &[SocketAddr],
    domain: &str,
    selector: &str,
) -> Result<Report, dns::DnsError> {
    let root = dns::txt_records(servers, domain).await?;
    let dkim = dns::txt_records(servers, &format!("{selector}._domainkey.{domain}"))
        .await
        .unwrap_or_default();
    let dmarc = dns::txt_records(servers, &format!("_dmarc.{domain}"))
        .await
        .unwrap_or_default();
    Ok(evaluate(&root, &dkim, &dmarc))
}

/// Bản ghi gợi ý cho tên miền gửi email.
pub fn recommended(
    domain: &str,
    selector: &str,
    spf_include: &str,
    report_to: &str,
) -> Vec<(String, String)> {
    vec![
        (
            domain.to_owned(),
            format!("v=spf1 include:{spf_include} -all"),
        ),
        (
            format!("{selector}._domainkey.{domain}"),
            "v=DKIM1; k=rsa; p=<khóa công khai do máy chủ chuyển tiếp cung cấp>".into(),
        ),
        (
            format!("_dmarc.{domain}"),
            format!("v=DMARC1; p=quarantine; rua=mailto:{report_to}; adkim=s; aspf=s"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checks_spf_dkim_dmarc_over_dns() {
        let records: dns::testing::Records = Default::default();
        let server = dns::testing::serve("127.0.0.1:0", records.clone())
            .await
            .unwrap();
        // Chưa có bản ghi nào: thiếu cả ba.
        let r = check(&[server], "f-eco.test", "feco1").await.unwrap();
        assert_eq!(r.problems.len(), 3, "{r:?}");
        {
            let mut m = records.lock().unwrap();
            m.insert(
                "f-eco.test".into(),
                vec![
                    "google-site-verification=x".into(),
                    "v=spf1 include:_spf.mail.example -all".into(),
                ],
            );
            m.insert(
                "feco1._domainkey.f-eco.test".into(),
                vec!["v=DKIM1; k=rsa; p=MIIBIjANBgkqh".into()],
            );
            m.insert(
                "_dmarc.f-eco.test".into(),
                vec!["v=DMARC1; p=none; rua=mailto:dmarc@f-eco.test".into()],
            );
        }
        let r = check(&[server], "f-eco.test", "feco1").await.unwrap();
        assert_eq!(
            r.problems,
            vec!["DMARC p=none: nên là quarantine hoặc reject khi đã gửi ổn định".to_owned()]
        );
        records.lock().unwrap().insert(
            "_dmarc.f-eco.test".into(),
            vec!["v=DMARC1; p=reject".into()],
        );
        let r = check(&[server], "f-eco.test", "feco1").await.unwrap();
        assert!(r.ok(), "{r:?}");
        assert!(r.spf.unwrap().contains("-all"));
    }

    #[test]
    fn revoked_dkim_and_double_spf_are_problems() {
        let r = evaluate(
            &["v=spf1 -all".into(), "v=spf1 ~all".into()],
            &["v=DKIM1; p=".into()],
            &["v=DMARC1; p=reject".into()],
        );
        assert_eq!(r.problems.len(), 2, "{r:?}");
        let rec = recommended("f-eco.vn", "feco1", "_spf.mail.example", "dmarc@f-eco.vn");
        assert_eq!(rec[2].0, "_dmarc.f-eco.vn");
    }
}
