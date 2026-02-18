use cargo_metadata::MetadataCommand;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

const CONFIG_FILE: &str = "cargo_deploy.json";

#[derive(Parser)]
struct Args {
    /// Build in release by default, unless debug is specified.
    #[arg(long)]
    debug: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    target_arch: Option<String>, // The instruction set of the remote device, for cross compiling.
    target_dest: Option<String>, // The remote folder to which the executable is copied.
    target_name: Option<String>, // The hostname/IP of the remote device.
    target_user: Option<String>, // The user on the remote device.
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_arch: Some("aarch64-unknown-linux-gnu".into()),
            target_dest: Some("/home/raspberry/bin".into()),
            target_name: None,
            target_user: None,
        }
    }
}

fn main() {
    let args = Args::parse();
    let release_mode = !args.debug;

    let mut config = load_or_create_config();
    let mut need_save = false;
    // Prompt for Hostname/IP if missing.
    if config.target_name.is_none() {
        print!("Enter remote hostname/IP : ");
        io::stdout().flush().unwrap();
        let mut ip = String::new();
        io::stdin().read_line(&mut ip).unwrap();
        config.target_name = Some(ip.trim().to_string());
        need_save = true;
    }

    if config.target_user.is_none() {
        print!("Enter remote username : ");
        io::stdout().flush().unwrap();
        let mut username = String::new();
        io::stdin().read_line(&mut username).unwrap();
        let username = username.trim().to_string();
        config.target_user = Some(username.clone());
        // Update the home directory based on the username.
        config.target_dest = Some(format!("/home/{}/bin", username));
        need_save = true;
    }

    if need_save {
        save_config(&config);
    }

    let target_arch = config
        .target_arch
        .clone()
        .unwrap_or_else(|| "aarch64-unknown-linux-gnu".into());

    build(&target_arch, release_mode);

    let target_name = config.target_name.clone().unwrap();
    let target_user = config.target_user.clone().unwrap();
    configure_ssh_key(&target_name, &target_user);

    let target_dest = config
        .target_dest
        .clone()
        .unwrap_or_else(|| "/home/raspberry/bin".into());

    create_remote_directory(&target_name, &target_user, &target_dest);
    let bin_name = detect_binary_name();
    let profile_dir: &str = if release_mode { "release" } else { "debug" };
    let binary_path = format!("target/{}/{}/{}", target_arch, profile_dir, bin_name);
    deploy(&binary_path, &target_name, &target_user, &target_dest);

    println!("Deployment complete.");
}

fn deploy(host_path: &str, target_name: &str, target_user: &str, target_dest: &str) {
    let connection_string = format!("{}@{}", target_user, target_name);
    println!("Uploading to {}:{}...", connection_string, target_dest);
    let status = Command::new("scp")
        .arg(&host_path)
        .arg(format!("{}:{}", connection_string, target_dest))
        .stdout(Stdio::null())
        .status()
        .expect("Failed to run SCP file transfer utility");

    if !status.success() {
        panic!(
            "SCP file transfer failed. Check your connection to {}",
            target_name
        );
    }
}

fn load_or_create_config() -> Config {
    let config_path = Path::new(CONFIG_FILE);
    if config_path.exists() {
        let data =
            fs::read_to_string(config_path).expect(&format!("Failed to read {}", CONFIG_FILE));
        serde_json::from_str(&data).expect(&format!("Invalid JSON in {}", CONFIG_FILE))
    } else {
        let default = Config::default();
        save_config(&default);
        println!("Created new deploy config file: {}", CONFIG_FILE);
        default
    }
}

fn save_config(config: &Config) {
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::write(CONFIG_FILE, json).expect(&format!("Failed to write {}", CONFIG_FILE));
}

pub fn sanitize_hostname(hostname: &str) -> String {
    let mut sanitized = String::with_capacity(hostname.len());

    for c in hostname.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }

    // Collapse multiple consecutive underscores
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }

    sanitized.trim_matches('_').to_string()
}

fn configure_ssh_key(target_name: &str, target_user: &str) {
    println!("Checking SSH connectivity...");

    let connection_string = format!("{}@{}", target_user, target_name);
    let test = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(&connection_string)
        .arg("echo connected")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Ok(status) = test {
        if status.success() {
            return;
        }
    }

    println!("No SSH key configured for {}.", connection_string);
    let home = std::env::var("HOME")
        .expect("HOME environment variable not set. Cannot locate '$HOME/.ssh/' on your machine.");

    let key_path = format!(
        "{}/.ssh/id_ed25519_{}_{}",
        home,
        target_user,
        sanitize_hostname(&target_name)
    );

    if !Path::new(&key_path).exists() {
        println!("No SSH key found on your machine. Generating one...");

        let comment = format!(
            "Key generated by {}, Version: {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        let status = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-f", &key_path, "-N", "", "-C", &comment])
            .status()
            .expect("Failed to generate SSH key");

        if !status.success() {
            panic!("SSH key generation failed");
        }
    }

    let status = Command::new("ssh-copy-id")
        .args(["-i", &key_path, &connection_string])
        .status()
        .expect("Failed to run ssh-copy-id");

    if !status.success() {
        panic!("ssh-copy-id failed");
    }
}

fn build(target_arch: &str, release: bool) {
    println!(
        "Building ({}) for {}...",
        if release { "release" } else { "debug" },
        target_arch
    );

    let mut cmd = Command::new("cross");
    cmd.args(["build", "--target", target_arch]);

    if release {
        cmd.arg("--release");
    }

    let status = cmd.status().expect("Failed to run cross build");
    if !status.success() {
        panic!("Build failed");
    }
}

fn detect_binary_name() -> String {
    let metadata = MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    let package = metadata.root_package().expect("No root package found");

    package
        .targets
        .iter()
        .find(|t| t.kind.contains(&"bin".into()))
        .expect("No binary target found")
        .name
        .clone()
}

fn create_remote_directory(target_name: &str, target_user: &str, target_dest: &str) {
    let connection_string = format!("{}@{}", target_user, target_name);

    let status = Command::new("ssh")
        .arg(connection_string)
        .arg(format!("mkdir -p {}", target_dest))
        .status()
        .expect("Failed to run ssh");

    if !status.success() {
        panic!("Failed to create remote directory");
    }
}
