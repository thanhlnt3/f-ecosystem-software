//! Nhận dạng giấy phép của mã nguồn: tệp `LICENSE*`/`LICENCE*`/`COPYING*` và trường `license`
//! của `Cargo.toml`, `package.json`, `pyproject.toml` ở thư mục gốc; so với giấy phép khai trong
//! `f-app.toml` (`license.mismatch`, mức `high`).

use std::collections::BTreeSet;

use crate::{Category, DetectedLicense, Finding, Severity};

/// Mã SPDX từ nội dung một tệp giấy phép.
pub fn from_text(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_lowercase();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |s: &str| t.contains(s);
    Some(if has("gnu affero general public license") {
        "AGPL-3.0"
    } else if has("gnu lesser general public license") {
        if has("version 2.1") {
            "LGPL-2.1"
        } else {
            "LGPL-3.0"
        }
    } else if has("gnu general public license") {
        if has("version 3") {
            "GPL-3.0"
        } else {
            "GPL-2.0"
        }
    } else if has("mozilla public license version 2.0")
        || has("mozilla public license, version 2.0")
    {
        "MPL-2.0"
    } else if has("apache license") && has("version 2.0") {
        "Apache-2.0"
    } else if has("this is free and unencumbered software released into the public domain") {
        "Unlicense"
    } else if has("permission is hereby granted, free of charge") {
        "MIT"
    } else if has(
        "permission to use, copy, modify, and/or distribute this software for any purpose",
    ) || has("isc license")
    {
        "ISC"
    } else if has("redistribution and use in source and binary forms") {
        if has("neither the name") || has("names of its contributors") {
            "BSD-3-Clause"
        } else {
            "BSD-2-Clause"
        }
    } else {
        return None;
    })
}

fn is_license_file(file: &str) -> bool {
    let up = file.to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING"]
        .iter()
        .any(|p| up.starts_with(p))
}

/// Giấy phép khai trong một tệp ở thư mục gốc (`path` không có `/`).
pub fn detect(path: &str, text: &str) -> Vec<DetectedLicense> {
    let found = |spdx: String| {
        vec![DetectedLicense {
            path: path.to_owned(),
            spdx,
        }]
    };
    match path {
        "Cargo.toml" | "pyproject.toml" => {
            let Ok(table) = toml::from_str::<toml::Table>(text) else {
                return Vec::new();
            };
            let candidates = [
                table.get("package").and_then(|p| p.get("license")),
                table
                    .get("workspace")
                    .and_then(|w| w.get("package"))
                    .and_then(|p| p.get("license")),
                table.get("project").and_then(|p| p.get("license")),
                table
                    .get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("license")),
            ];
            for c in candidates.into_iter().flatten() {
                let value = match c {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => t.get("text").and_then(|v| v.as_str()).map(|s| {
                        from_text(s)
                            .map(str::to_owned)
                            .unwrap_or_else(|| s.trim().to_owned())
                    }),
                    _ => None,
                };
                if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
                    return found(v);
                }
            }
            Vec::new()
        }
        "package.json" => {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
                return Vec::new();
            };
            let value = json["license"]
                .as_str()
                .or_else(|| json["license"]["type"].as_str());
            match value {
                Some(v) if !v.trim().is_empty() && !v.eq_ignore_ascii_case("UNLICENSED") => {
                    found(v.trim().to_owned())
                }
                _ => Vec::new(),
            }
        }
        f if is_license_file(f) => from_text(text)
            .map(|s| found(s.to_owned()))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Các mã SPDX trong một biểu thức (`MIT OR Apache-2.0`), đã chuẩn hóa để so sánh.
pub fn ids(expression: &str) -> BTreeSet<String> {
    expression
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '/')
        .filter(|w| {
            !w.is_empty() && !["OR", "AND", "WITH"].contains(&w.to_ascii_uppercase().as_str())
        })
        .map(|w| {
            let u = w.to_ascii_uppercase();
            let u = u.trim_end_matches('+');
            let u = u.strip_suffix("-ONLY").unwrap_or(u);
            let u = u.strip_suffix("-OR-LATER").unwrap_or(u);
            match u {
                "APACHE2" | "APACHE-2" | "APACHE" => "APACHE-2.0".to_owned(),
                "BSD-3" | "BSD3" => "BSD-3-CLAUSE".to_owned(),
                "BSD-2" | "BSD2" => "BSD-2-CLAUSE".to_owned(),
                "GPL-3" | "GPLV3" => "GPL-3.0".to_owned(),
                "GPL-2" | "GPLV2" => "GPL-2.0".to_owned(),
                other => other.to_owned(),
            }
        })
        .collect()
}

/// `license.mismatch` khi giấy phép trong mã nguồn không có trong giấy phép khai ở `f-app.toml`.
pub fn mismatch(declared: &str, detected: &[DetectedLicense]) -> Option<Finding> {
    if detected.is_empty() {
        return None;
    }
    let want = ids(declared);
    let found: BTreeSet<String> = detected.iter().flat_map(|d| ids(&d.spdx)).collect();
    if want.intersection(&found).next().is_some() {
        return None;
    }
    let list = detected
        .iter()
        .map(|d| format!("{} ({})", d.spdx, d.path))
        .collect::<Vec<_>>()
        .join(", ");
    Some(Finding {
        rule: "license.mismatch".into(),
        category: Category::License,
        severity: Severity::High,
        path: detected[0].path.clone(),
        line: 0,
        message: format!("f-app.toml khai giấy phép {declared}, mã nguồn khai {list}"),
        excerpt: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_texts() {
        assert_eq!(
            from_text("MIT License\n\nPermission is hereby granted, free of charge, to any person"),
            Some("MIT")
        );
        assert_eq!(
            from_text("                 Apache License\n           Version 2.0, January 2004"),
            Some("Apache-2.0")
        );
        assert_eq!(
            from_text("GNU AFFERO GENERAL PUBLIC LICENSE\nVersion 3, 19 November 2007"),
            Some("AGPL-3.0")
        );
        assert_eq!(
            from_text("GNU GENERAL PUBLIC LICENSE\n Version 3, 29 June 2007"),
            Some("GPL-3.0")
        );
        assert_eq!(
            from_text("Redistribution and use in source and binary forms ... Neither the name of"),
            Some("BSD-3-Clause")
        );
        assert_eq!(from_text("Mọi quyền được bảo lưu"), None);
    }

    #[test]
    fn manifests() {
        let cargo = "[package]\nname = \"x\"\nlicense = \"MIT OR Apache-2.0\"\n";
        assert_eq!(detect("Cargo.toml", cargo)[0].spdx, "MIT OR Apache-2.0");
        assert_eq!(
            detect("package.json", r#"{"license": "ISC"}"#)[0].spdx,
            "ISC"
        );
        assert!(detect("package.json", r#"{"license": "UNLICENSED"}"#).is_empty());
        let py = "[project]\nname = \"x\"\nlicense = {text = \"MIT\"}\n";
        assert_eq!(detect("pyproject.toml", py)[0].spdx, "MIT");
        assert_eq!(
            detect("LICENSE-APACHE", "Apache License\nVersion 2.0")[0].spdx,
            "Apache-2.0"
        );
        assert!(detect("README.md", "MIT License").is_empty());
    }

    #[test]
    fn mismatch_against_manifest() {
        let mit = vec![DetectedLicense {
            path: "LICENSE".into(),
            spdx: "MIT".into(),
        }];
        assert!(mismatch("MIT OR Apache-2.0", &mit).is_none());
        assert!(mismatch("mit", &mit).is_none());
        let gpl = vec![DetectedLicense {
            path: "COPYING".into(),
            spdx: "GPL-3.0".into(),
        }];
        assert!(mismatch("GPL-3.0-or-later", &gpl).is_none());
        let f = mismatch("MIT", &gpl).unwrap();
        assert_eq!(
            (f.rule.as_str(), f.severity),
            ("license.mismatch", Severity::High)
        );
        assert!(f.message.contains("GPL-3.0 (COPYING)"), "{}", f.message);
        assert!(mismatch("MIT", &[]).is_none());
    }
}
