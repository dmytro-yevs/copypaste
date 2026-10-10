use std::path::Path;

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--inference-worker")
    {
        if copypaste_modules::run_inference_worker(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
        )
        .is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    let arguments: Vec<String> = std::env::args().collect();
    let result = if arguments.len() == 4 && arguments[1] == "--finish-removal" {
        copypaste_module_qualification::finish_after_restart(
            Path::new(&arguments[2]),
            &arguments[3],
        )
        .map(|()| "{\"removal_completed_after_restart\":true}".to_owned())
    } else if arguments.len() == 7 {
        copypaste_module_qualification::qualify(
            Path::new(&arguments[1]),
            Path::new(&arguments[2]),
            Path::new(&arguments[3]),
            &arguments[4],
            &arguments[5],
            &arguments[6],
        )
        .and_then(|receipt| serde_json::to_string(&receipt).map_err(|error| error.to_string()))
    } else {
        Err(
            "usage: qualify <package> <fixtures> <private-data> <app-version> <commit> <run-id>"
                .into(),
        )
    };
    match result {
        Ok(receipt) => println!("{receipt}"),
        Err(error) => {
            eprintln!("Module qualification failed: {error}");
            std::process::exit(1);
        }
    }
}
