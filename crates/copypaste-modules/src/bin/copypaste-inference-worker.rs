fn main() {
    if copypaste_modules::run_inference_worker(std::io::stdin().lock(), std::io::stdout().lock())
        .is_err()
    {
        std::process::exit(1);
    }
}
