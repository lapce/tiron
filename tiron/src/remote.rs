use std::{
    io::BufReader,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Result};
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tiron_common::{action::ActionMessage, node::NodeMessage};
use tiron_node::stdio::stdio_transport;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct SshHost {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<usize>,
}

impl SshHost {
    pub fn user_host(&self) -> String {
        if let Some(user) = self.user.as_ref() {
            format!("{user}@{}", self.host)
        } else {
            self.host.clone()
        }
    }
}

pub struct SshRemote {
    pub ssh: SshHost,
}

impl SshRemote {
    #[cfg(windows)]
    const SSH_ARGS: &'static [&'static str] = &[];

    #[cfg(unix)]
    const SSH_ARGS: &'static [&'static str] = &[
        "-o",
        "ControlMaster=auto",
        "-o",
        "ControlPath=~/.ssh/cm_%C",
        "-o",
        "ControlPersist=30m",
        "-o",
        "ConnectTimeout=15",
    ];

    fn command_builder(&self) -> Command {
        let mut cmd = Self::new_command("ssh");
        cmd.args(Self::SSH_ARGS);

        if let Some(port) = self.ssh.port {
            cmd.arg("-p").arg(port.to_string());
        }

        cmd.arg(self.ssh.user_host());

        if !std::env::var("TIRON_DEBUG").unwrap_or_default().is_empty() {
            cmd.arg("-v");
        }

        cmd
    }

    fn new_command(program: &str) -> Command {
        #[allow(unused_mut)]
        let mut cmd = Command::new(program);
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        cmd
    }
}

pub fn start_remote(
    remote: SshRemote,
    sudo: bool,
) -> Result<(Sender<NodeMessage>, Receiver<ActionMessage>)> {
    let (platform, architecture) = host_specification(&remote)?;

    if platform == HostPlatform::UnknownOS {
        return Err(anyhow!("Unknown OS"));
    }

    if architecture == HostArchitecture::UnknownArch {
        return Err(anyhow!("Unknown architecture"));
    }

    // ! Below paths have to be synced with what is
    // ! returned by Config::proxy_directory()
    let tiron_node_path = match platform {
        HostPlatform::Windows => "%HOMEDRIVE%%HOMEPATH%\\AppData\\Local\\tiron\\tiron\\data",
        HostPlatform::Darwin => "~/Library/Application\\ Support/dev.tiron.tiron",
        _ => "~/.local/share/tiron",
    };

    let tiron_node_file = match platform {
        HostPlatform::Windows => {
            format!(
                "{tiron_node_path}\\tiron-node-{}.exe",
                env!("CARGO_PKG_VERSION")
            )
        }
        _ => format!("{tiron_node_path}/tiron-node-{}", env!("CARGO_PKG_VERSION")),
    };

    if !remote
        .command_builder()
        .args([&tiron_node_file, "--version"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).trim()
                == format!("tiron-node {}", env!("CARGO_PKG_VERSION"))
        })
        .unwrap_or(false)
    {
        download_remote(
            &remote,
            &platform,
            &architecture,
            tiron_node_path,
            &tiron_node_file,
        )?;
    };

    let mut child = match platform {
        // Force cmd.exe usage to resolve %envvar% variables
        HostPlatform::Windows => remote
            .command_builder()
            .args(["cmd", "/c"])
            .arg(&tiron_node_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
        _ => {
            let mut cmd = remote.command_builder();
            if sudo {
                cmd.arg("sudo");
            }
            cmd.arg(&tiron_node_file)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?
        }
    };
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("can't find stdin"))?;
    let stdout = BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("can't find stdout"))?,
    );

    let (writer_tx, writer_rx) = crossbeam_channel::unbounded::<NodeMessage>();
    let (reader_tx, reader_rx) = crossbeam_channel::unbounded::<ActionMessage>();
    stdio_transport(stdin, writer_rx, stdout, reader_tx);

    Ok((writer_tx, reader_rx))
}

fn download_remote(
    remote: &SshRemote,
    platform: &HostPlatform,
    architecture: &HostArchitecture,
    tiron_node_path: &str,
    tiron_node_file: &str,
) -> Result<()> {
    let url = format!(
        "https://github.com/lapce/tiron/releases/download/v{}/tiron-node-{}-{platform}-{architecture}.gz",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_VERSION")
    );
    remote
        .command_builder()
        .args([
            "mkdir",
            "-p",
            tiron_node_path,
            "&&",
            "curl",
            "-L",
            &url,
            "|",
            "gzip",
            "-d",
            ">",
            tiron_node_file,
            "&&",
            "chmod",
            "+x",
            tiron_node_file,
        ])
        .output()?;
    Ok(())
}

fn host_specification(remote: &SshRemote) -> Result<(HostPlatform, HostArchitecture)> {
    use HostArchitecture::*;
    use HostPlatform::*;

    let cmd = remote.command_builder().args(["uname", "-sm"]).output();

    let spec = match cmd {
        Ok(cmd) => {
            let stdout = String::from_utf8_lossy(&cmd.stdout).to_lowercase();
            let stdout = stdout.trim();
            match stdout {
                // If empty, then we probably deal with Windows and not Unix
                // or something went wrong with command output
                "" => {
                    let (os, arch) = host_specification_try_windows(remote)?;
                    if os != UnknownOS && arch != UnknownArch {
                        (os, arch)
                    } else {
                        return Err(anyhow!(String::from_utf8_lossy(&cmd.stderr).to_string()));
                    }
                }
                v => {
                    if let Some((os, arch)) = v.split_once(' ') {
                        let os = parse_os(os);
                        let arch = parse_arch(arch);
                        if os == UnknownOS || arch == UnknownArch {
                            return Err(anyhow!(v.to_string()));
                        }
                        (os, arch)
                    } else {
                        return Err(anyhow!(v.to_string()));
                    }
                }
            }
        }
        Err(e) => return Err(anyhow!(e)),
    };
    Ok(spec)
}

fn host_specification_try_windows(remote: &SshRemote) -> Result<(HostPlatform, HostArchitecture)> {
    use HostArchitecture::*;
    use HostPlatform::*;
    // Try cmd explicitly
    let cmd = remote
        .command_builder()
        .args(["cmd", "/c", "echo %OS% %PROCESSOR_ARCHITECTURE%"])
        .output();
    let spec = match cmd {
        Ok(cmd) => {
            let stdout = String::from_utf8_lossy(&cmd.stdout).to_lowercase();
            let stdout = stdout.trim();
            match stdout.split_once(' ') {
                Some((os, arch)) => (parse_os(os), parse_arch(arch)),
                None => {
                    // PowerShell fallback
                    let cmd = remote
                        .command_builder()
                        .args(["echo", "\"${env:OS} ${env:PROCESSOR_ARCHITECTURE}\""])
                        .output();
                    match cmd {
                        Ok(cmd) => {
                            let stdout = String::from_utf8_lossy(&cmd.stdout).to_lowercase();
                            let stdout = stdout.trim();
                            match stdout.split_once(' ') {
                                Some((os, arch)) => (parse_os(os), parse_arch(arch)),
                                None => (UnknownOS, UnknownArch),
                            }
                        }
                        Err(_) => (UnknownOS, UnknownArch),
                    }
                }
            }
        }
        Err(_) => (UnknownOS, UnknownArch),
    };
    Ok(spec)
}

fn parse_arch(arch: &str) -> HostArchitecture {
    use HostArchitecture::*;
    // processor architectures be like that
    match arch.to_lowercase().as_str() {
        "amd64" | "x64" | "x86_64" => AMD64,
        "x86" | "i386" | "i586" | "i686" => X86,
        "arm" | "armhf" | "armv6" => ARM32v6,
        "armv7" | "armv7l" => ARM32v7,
        "arm64" | "armv8" | "aarch64" => ARM64,
        _ => UnknownArch,
    }
}

fn parse_os(os: &str) -> HostPlatform {
    use HostPlatform::*;
    match os.to_lowercase().as_str() {
        "linux" => Linux,
        "darwin" => Darwin,
        "windows_nt" => Windows,
        v if v.ends_with("bsd") => Bsd,
        _ => UnknownOS,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::Display)]
#[strum(ascii_case_insensitive)]
enum HostPlatform {
    UnknownOS,
    #[strum(serialize = "windows")]
    Windows,
    #[strum(serialize = "linux")]
    Linux,
    #[strum(serialize = "darwin")]
    Darwin,
    #[strum(serialize = "bsd")]
    Bsd,
}

/// serialise via strum to arch name that is used
/// in CI artefacts
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::Display)]
#[strum(ascii_case_insensitive)]
enum HostArchitecture {
    UnknownArch,
    #[strum(serialize = "amd64")]
    AMD64,
    #[strum(serialize = "x86")]
    X86,
    #[strum(serialize = "arm64")]
    ARM64,
    #[strum(serialize = "armv7")]
    ARM32v7,
    #[strum(serialize = "armhf")]
    ARM32v6,
}
