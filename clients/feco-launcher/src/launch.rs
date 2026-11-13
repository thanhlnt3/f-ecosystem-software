//! Mở app con (spec launcher, "Mở app con và truyền phiên đăng nhập"; design D4).
//!
//! App nhận qua biến môi trường:
//! - `FECO_LAUNCH_CODE`: mã dùng một lần, sống 60 giây, đổi lấy token tại
//!   `POST {FECO_SERVER_URL}v1/launch-codes/exchange`.
//! - `FECO_CLIENT_ID`, `FECO_SERVER_URL`, `FECO_ISSUER_URL`, `FECO_APP_DATA_DIR` (F-SDK đọc các
//!   biến này).
//! - `FECO_LAUNCHER_URL`, `FECO_LAUNCHER_SECRET`: API cục bộ để xin mã mới (xem [`crate::broker`]).
//!
//! Mọi biến `FECO_*` khác của launcher bị gỡ khỏi môi trường của app.

use crate::install::LaunchPlan;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct LaunchEnv {
    pub launch_code: Option<String>,
    pub server_url: String,
    /// Máy chủ OAuth 2.0 / OIDC (`FECO_ISSUER_URL`).
    pub issuer_url: String,
    pub broker_url: Option<String>,
    pub broker_secret: Option<String>,
}

/// Chương trình và tham số để mở `program` trong một cửa sổ terminal riêng trên hệ điều hành
/// `os` (design E5, improve-launcher-experience).
///
/// Windows dùng `cmd /C start "" <program>`: `start` tạo cửa sổ console mới với stdin/stdout của
/// riêng nó. Chỉ đặt cờ `CREATE_NEW_CONSOLE` thì app vẫn nhận stdio launcher thừa hưởng (ví dụ
/// file nhật ký) và app terminal báo "handle is invalid".
pub fn terminal_wrapper(
    os: &str,
    program: &std::path::Path,
    terminal_env: Option<&str>,
) -> (String, Vec<std::ffi::OsString>) {
    match os {
        "windows" => (
            "cmd".into(),
            vec!["/C".into(), "start".into(), "".into(), program.into()],
        ),
        // Terminal.app mở file chạy trong cửa sổ mới; biến môi trường không đi qua `open`.
        "macos" => (
            "open".into(),
            vec!["-a".into(), "Terminal".into(), program.into()],
        ),
        _ => (
            terminal_env
                .filter(|t| !t.trim().is_empty())
                .unwrap_or("x-terminal-emulator")
                .to_owned(),
            vec!["-e".into(), program.into()],
        ),
    }
}

/// Cờ của Windows để chính `cmd` (chỉ dùng để gọi `start`) không hiện cửa sổ.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn command(plan: &LaunchPlan, env: &LaunchEnv) -> Command {
    let mut cmd = if plan.terminal {
        let (program, args) = terminal_wrapper(
            std::env::consts::OS,
            &plan.program,
            std::env::var("TERMINAL").ok().as_deref(),
        );
        let mut c = Command::new(program);
        c.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        c
    } else {
        Command::new(&plan.program)
    };
    cmd.current_dir(&plan.working_dir);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("FECO_") {
            cmd.env_remove(key);
        }
    }
    cmd.env("FECO_APP_DATA_DIR", &plan.data_dir);
    cmd.env("FECO_SERVER_URL", &env.server_url);
    cmd.env("FECO_ISSUER_URL", &env.issuer_url);
    if let Some(client_id) = &plan.client_id {
        cmd.env("FECO_CLIENT_ID", client_id);
    }
    if let Some(code) = &env.launch_code {
        cmd.env("FECO_LAUNCH_CODE", code);
    }
    if let (Some(url), Some(secret)) = (&env.broker_url, &env.broker_secret) {
        cmd.env("FECO_LAUNCHER_URL", url);
        cmd.env("FECO_LAUNCHER_SECRET", secret);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Scenario "Mở trình soạn thảo trong terminal": lệnh mở theo từng hệ điều hành.
    #[test]
    fn terminal_apps_open_in_a_terminal_window() {
        let program = Path::new("/apps/kibi/0.3.3/bin/kibi");
        let (prog, args) = terminal_wrapper("windows", program, None);
        assert_eq!(prog, "cmd");
        assert_eq!(args, ["/C", "start", "", "/apps/kibi/0.3.3/bin/kibi"]);
        let (prog, args) = terminal_wrapper("macos", program, None);
        assert_eq!(prog, "open");
        assert_eq!(args, ["-a", "Terminal", "/apps/kibi/0.3.3/bin/kibi"]);
        let (prog, args) = terminal_wrapper("linux", program, None);
        assert_eq!(prog, "x-terminal-emulator");
        assert_eq!(args, ["-e", "/apps/kibi/0.3.3/bin/kibi"]);
        let (prog, _) = terminal_wrapper("linux", program, Some("alacritty"));
        assert_eq!(prog, "alacritty");
    }

    #[test]
    fn non_terminal_apps_run_directly() {
        let plan = LaunchPlan {
            app: "notes".into(),
            title: "Ghi chú".into(),
            version: "1.0.0".into(),
            program: "notes-bin".into(),
            working_dir: ".".into(),
            data_dir: "data".into(),
            client_id: None,
            terminal: false,
        };
        let cmd = command(&plan, &LaunchEnv::default());
        assert_eq!(cmd.get_program(), "notes-bin");
        let plan = LaunchPlan {
            terminal: true,
            ..plan
        };
        let cmd = command(&plan, &LaunchEnv::default());
        assert_ne!(cmd.get_program(), "notes-bin");
        assert_eq!(cmd.get_current_dir(), Some(Path::new(".")));
        // Biến môi trường (thư mục dữ liệu, máy chủ...) vẫn tới được app qua `start`/`open`.
        assert!(cmd
            .get_envs()
            .any(|(k, v)| k == "FECO_APP_DATA_DIR" && v == Some(std::ffi::OsStr::new("data"))));
    }
}
