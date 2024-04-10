use std::process::ExitStatus;

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use tiron_common::action::{ActionId, ActionMessage};

use crate::action::command::run_command;

use super::PackageState;

pub trait Provider {
    fn install(&self);
}

pub enum PackageProvider {
    Apt,
    Dnf,
    Pacman,
    Homebrew,
    Winget,
    Zypper,
}

impl PackageProvider {
    pub fn detect() -> Result<Self> {
        use os_info::Type;

        let info = os_info::get();
        let os_type = info.os_type();
        let provider = match info.os_type() {
            Type::Arch => Self::Pacman,
            Type::Manjaro => Self::Pacman,

            Type::Debian => Self::Apt,
            Type::Mint => Self::Apt,
            Type::Pop => Self::Apt,
            Type::Ubuntu => Self::Apt,
            Type::OracleLinux => Self::Apt,

            Type::Fedora => Self::Dnf,
            Type::Redhat => Self::Dnf,
            Type::RedHatEnterprise => Self::Dnf,
            Type::CentOS => Self::Dnf,

            Type::openSUSE => Self::Zypper,
            Type::SUSE => Self::Zypper,

            Type::Macos => Self::Homebrew,

            Type::Windows => Self::Winget,

            _ => return Err(anyhow!("Can't find the package manger for OS {os_type}")),
        };

        Ok(provider)
    }

    pub fn run(
        &self,
        id: ActionId,
        tx: &Sender<ActionMessage>,
        packages: Vec<String>,
        state: PackageState,
    ) -> Result<ExitStatus> {
        let cmd = match state {
            PackageState::Present => "install",
            PackageState::Absent => "remove",
            PackageState::Latest => "upgrade",
        };

        let (program, args) = match self {
            PackageProvider::Apt => ("apt", vec![cmd, "--yes"]),
            PackageProvider::Dnf => ("dnf", vec![cmd, "--assumeyes"]),
            PackageProvider::Pacman => (
                "yay",
                vec![cmd, "--noconfirm", "--nocleanmenu", "--nodiffmenu"],
            ),
            PackageProvider::Homebrew => ("brew", vec![cmd]),
            PackageProvider::Winget => (
                "winget",
                vec![
                    cmd,
                    "--silent",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                    "--source",
                    "winget",
                ],
            ),
            PackageProvider::Zypper => ("zypper", vec![cmd, "-y"]),
        };

        let mut args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        args.extend_from_slice(&packages);

        let status = run_command(id, tx, program, &args)?;
        Ok(status)
    }
}
