use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

fn dispatch() -> Result<(), String> {
    match env::args().nth(1).as_deref().unwrap_or("help") {
        "apk" => build_apk(),
        "install" => {
            build_apk()?;
            open_apk()
        }
        "open" => open_apk(),
        "help" => {
            println!("cargo run --manifest-path xtask/Cargo.toml -- <apk|install|open>");
            Ok(())
        }
        other => Err(format!(
            "unknown command `{other}`; use apk, install, or open"
        )),
    }
}

fn build_apk() -> Result<(), String> {
    run(
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(root())
            .args([
                "build",
                "--lib",
                "--locked",
                "--target",
                "aarch64-linux-android",
            ]),
    )?;
    run(Command::new("bash").arg("package.sh").current_dir(root()))
}

fn open_apk() -> Result<(), String> {
    let apk = root().join("dist/gpui-kit-demo.apk");
    if !apk.is_file() {
        return Err("APK not found; use `xtask install` to build it first".into());
    }
    // Termux's content provider cannot share paths inside the PRoot filesystem.
    // Stage the APK in Termux's real home before opening the installer.
    let termux_home = PathBuf::from("/data/data/com.termux/files/home");
    let shared_apk = if termux_home.is_dir() {
        let destination = termux_home.join("gpui-kit-demo.apk");
        std::fs::copy(&apk, &destination)
            .map_err(|error| format!("could not stage APK: {error}"))?;
        destination
    } else {
        apk
    };
    let bundled_open = PathBuf::from("/data/data/com.termux/files/usr/bin/termux-open");
    let opener = if bundled_open.is_file() {
        bundled_open
    } else {
        "termux-open".into()
    };
    println!("Opening Android's installer for {}", shared_apk.display());
    run(Command::new(opener)
        .args([
            "--view",
            "--content-type",
            "application/vnd.android.package-archive",
        ])
        .arg(shared_apk))
}

fn run(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}
