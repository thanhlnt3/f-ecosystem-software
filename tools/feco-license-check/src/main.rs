use std::{fs, path::PathBuf, process::ExitCode};

use feco_license_check::{check, compose_images, ExceptionFile, NativeManifest};

fn main() -> ExitCode {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let read = |rel: &str| {
        fs::read_to_string(root.join(rel)).map_err(|e| format!("không đọc được {rel}: {e}"))
    };

    let result = (|| -> Result<_, String> {
        let native: NativeManifest = toml::from_str(&read("third_party/native.toml")?)
            .map_err(|e| format!("third_party/native.toml: {e}"))?;
        let exceptions: ExceptionFile =
            toml::from_str(&read("third_party/license-exceptions.toml")?)
                .map_err(|e| format!("third_party/license-exceptions.toml: {e}"))?;
        let images = compose_images(&read("infra/docker-compose.yml")?);
        Ok((
            native.components.len(),
            images.len(),
            check(&native, &exceptions, &images),
        ))
    })();

    match result {
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
        Ok((n, i, violations)) if violations.is_empty() => {
            println!("license: {n} thành phần, {i} ảnh container — đạt");
            ExitCode::SUCCESS
        }
        Ok((n, _, violations)) => {
            eprintln!(
                "license: {} vi phạm trong {n} thành phần:",
                violations.len()
            );
            for v in violations {
                eprintln!("  - {}", v.0);
            }
            ExitCode::FAILURE
        }
    }
}
