use std::{env, error, fs, path::Path};

type BuildResult<T> = Result<T, Box<dyn error::Error>>;

fn compile_resources() -> BuildResult<()> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let template = fs::read_to_string("assets/miniraw.rc.in")?;
    let parts = VERSION.split('.').collect::<Vec<_>>();
    let app_version_windows = if parts.len() >= 3 {
        format!("{},{},{},0", parts[0], parts[1], parts[2])
    } else {
        "0,0,0,0".to_owned()
    };
    let rc = template
        .replace("@APP_VERSION_WINDOWS@", &app_version_windows)
        .replace("@APP_VERSION@", VERSION)
        .replace(
            "@ROOT@",
            &env::var("CARGO_MANIFEST_DIR")?.replace("\\", "/"),
        );

    let rc_path = Path::new(&env::var("OUT_DIR")?).join("miniraw.rc");
    fs::write(&rc_path, rc.as_bytes())?;

    embed_resource::compile(rc_path);

    Ok(())
}

fn main() -> BuildResult<()> {
    compile_resources()?;
    Ok(())
}
