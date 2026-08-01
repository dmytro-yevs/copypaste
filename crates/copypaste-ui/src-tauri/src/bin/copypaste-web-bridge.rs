#[tokio::main]
async fn main() {
    if let Err(error) = copypaste_ui_lib::dev_web_bridge::run_from_env().await {
        eprintln!("CopyPaste browser bridge could not start: {error}");
        std::process::exit(1);
    }
}
