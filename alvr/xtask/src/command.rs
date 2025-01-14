use alvr_filesystem as afs;
use std::{
    env,
    error::Error,
    fmt::{self, Display},
    fs,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Debug)]
struct StringError(String);

impl Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for StringError {}

pub fn run_as_shell_in(
    workdir: &Path,
    shell: &str,
    shell_flag: &str,
    cmd: &str,
) -> Result<(), Box<dyn Error>> {
    println!(
        "\n[xtask - {}] > {cmd}",
        workdir.file_name().unwrap().to_string_lossy()
    );

    let output = Command::new(shell)
        .args(&[shell_flag, cmd])
        .stdout(Stdio::inherit())
        .current_dir(workdir)
        .spawn()?
        .wait_with_output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Box::new(StringError(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))))
    }
}

pub fn run_in(workdir: &Path, cmd: &str) -> Result<(), Box<dyn Error>> {
    let shell = if cfg!(windows) { "cmd" } else { "bash" };
    let shell_flag = if cfg!(windows) { "/C" } else { "-c" };

    run_as_shell_in(workdir, shell, shell_flag, cmd)
}

pub fn run(cmd: &str) -> Result<(), Box<dyn Error>> {
    run_in(&env::current_dir().unwrap(), cmd)
}

// Bash can be invoked on Windows if WSL is installed
pub fn run_as_bash_in(workdir: &Path, cmd: &str) -> Result<(), Box<dyn Error>> {
    run_as_shell_in(workdir, "bash", "-c", cmd)
}

pub fn run_without_shell(cmd: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    println!(
        "\n[xtask] > {}",
        args.iter().fold(String::from(cmd), |s, arg| s + " " + arg)
    );
    let output = Command::new(cmd)
        .args(args)
        .stdout(Stdio::inherit())
        .spawn()?
        .wait_with_output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Box::new(StringError(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))))
    }
}

pub fn zip(source: &Path) -> Result<(), Box<dyn Error>> {
    if cfg!(windows) {
        run_without_shell(
            "powershell",
            &[
                "Compress-Archive",
                &source.to_string_lossy(),
                &format!("{}.zip", source.to_string_lossy()),
            ],
        )
    } else {
        run_without_shell(
            "zip",
            &[
                "-r",
                &format!("{}.zip", source.to_string_lossy()),
                &source.to_string_lossy(),
            ],
        )
    }
}

pub fn unzip(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    if cfg!(windows) {
        run_without_shell(
            "powershell",
            &[
                "Expand-Archive",
                &source.to_string_lossy(),
                &destination.to_string_lossy(),
            ],
        )
    } else {
        run_without_shell(
            "unzip",
            &[
                &source.to_string_lossy(),
                "-d",
                &destination.to_string_lossy(),
            ],
        )
    }
}

pub fn download(url: &str, destination: &Path) -> Result<(), Box<dyn Error>> {
    run_without_shell(
        "curl",
        &["-x", "socks5://127.0.0.1:10808", "-L", "-o", &destination.to_string_lossy(), "--url", url],
    )
}

pub fn download_and_extract_zip(url: &str, destination: &Path) {
    let zip_file = afs::build_dir().join("temp_download.zip");

    fs::remove_file(&zip_file).ok();
    fs::create_dir_all(afs::build_dir()).unwrap();
    download(url, &zip_file).unwrap();

    fs::remove_dir_all(&destination).ok();
    fs::create_dir_all(&destination).unwrap();
    unzip(&zip_file, destination).unwrap();

    fs::remove_file(zip_file).unwrap();
}

pub fn date_utc_yyyymmdd() -> String {
    let output = if cfg!(windows) {
        Command::new("powershell")
            .arg("(Get-Date).ToUniversalTime().ToString(\"yyyy.MM.dd\")")
            .output()
            .unwrap()
    } else {
        Command::new("date")
            .args(&["-u", "+%Y.%m.%d"])
            .output()
            .unwrap()
    };

    String::from_utf8_lossy(&output.stdout)
        .as_ref()
        .to_owned()
        .replace('\r', "")
        .replace('\n', "")
}
