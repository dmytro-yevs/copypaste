use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let out_dir = PathBuf::from(args.next().ok_or("expected one output directory")?);
    if args.next().is_some() {
        return Err("expected exactly one output directory".into());
    }
    copypaste_ui_lib::typescript::export(out_dir)?;
    Ok(())
}
