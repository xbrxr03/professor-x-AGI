use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellSandbox {
    Bubblewrap,
    PolicyOnly,
}

impl ShellSandbox {
    pub fn label(self) -> &'static str {
        match self {
            ShellSandbox::Bubblewrap => "bubblewrap",
            ShellSandbox::PolicyOnly => "fallback-policy-only",
        }
    }
}

static BWRAP_USABLE: OnceLock<bool> = OnceLock::new();

pub fn selected_shell_sandbox() -> ShellSandbox {
    if *BWRAP_USABLE.get_or_init(probe_bwrap) {
        ShellSandbox::Bubblewrap
    } else {
        ShellSandbox::PolicyOnly
    }
}

pub fn restricted_shell_command(
    workspace_root: &Path,
    command: &str,
) -> (tokio::process::Command, ShellSandbox) {
    let sandbox = selected_shell_sandbox();
    let mut cmd = match sandbox {
        ShellSandbox::Bubblewrap => bubblewrap_command(workspace_root, command),
        ShellSandbox::PolicyOnly => policy_only_command(workspace_root, command),
    };
    cmd.current_dir(workspace_root)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    (cmd, sandbox)
}

fn policy_only_command(workspace_root: &Path, command: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command).current_dir(workspace_root);
    cmd
}

fn bubblewrap_command(workspace_root: &Path, command: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("bwrap");
    cmd.arg("--unshare-all")
        .arg("--die-with-parent")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr")
        .arg("--ro-bind")
        .arg("/bin")
        .arg("/bin")
        .arg("--ro-bind")
        .arg("/lib")
        .arg("/lib")
        .arg("--ro-bind-try")
        .arg("/lib64")
        .arg("/lib64")
        .arg("--bind")
        .arg(workspace_root)
        .arg(workspace_root)
        .arg("--chdir")
        .arg(workspace_root)
        .arg("sh")
        .arg("-c")
        .arg(command);
    cmd
}

fn probe_bwrap() -> bool {
    let Some(bwrap) = which_bwrap() else {
        return false;
    };
    std::process::Command::new(bwrap)
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr")
        .arg("--ro-bind")
        .arg("/bin")
        .arg("/bin")
        .arg("--ro-bind")
        .arg("/lib")
        .arg("/lib")
        .arg("--ro-bind-try")
        .arg("/lib64")
        .arg("/lib64")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--chdir")
        .arg("/")
        .arg("sh")
        .arg("-c")
        .arg("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn which_bwrap() -> Option<&'static str> {
    ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn sandbox_labels_are_operator_readable() {
        assert_eq!(ShellSandbox::Bubblewrap.label(), "bubblewrap");
        assert_eq!(ShellSandbox::PolicyOnly.label(), "fallback-policy-only");
    }

    #[test]
    fn bubblewrap_command_binds_workspace_and_runs_shell() {
        let workspace = Path::new("/tmp/prof-x-test-workspace");
        let cmd = bubblewrap_command(workspace, "cargo check");
        let args = cmd.as_std().get_args().collect::<Vec<_>>();
        assert_eq!(cmd.as_std().get_program(), OsStr::new("bwrap"));
        assert!(args.contains(&OsStr::new("--bind")));
        assert!(args.contains(&workspace.as_os_str()));
        assert!(args.contains(&OsStr::new("--chdir")));
        assert!(args.contains(&OsStr::new("sh")));
        assert!(args.contains(&OsStr::new("cargo check")));
    }

    #[test]
    fn policy_only_command_uses_workspace_shell() {
        let workspace = Path::new("/tmp/prof-x-test-workspace");
        let cmd = policy_only_command(workspace, "git status");
        assert_eq!(cmd.as_std().get_program(), OsStr::new("sh"));
        assert_eq!(cmd.as_std().get_current_dir(), Some(workspace));
        let args = cmd.as_std().get_args().collect::<Vec<_>>();
        assert_eq!(args, vec![OsStr::new("-c"), OsStr::new("git status")]);
    }
}
