//! Tệp chạy của `f-dev`; xem thư viện `f_dev` cho cách dùng.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match f_dev::run(&args).await {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("lỗi: {e}");
            ExitCode::FAILURE
        }
    }
}
