use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::APP_ID;

pub struct Autostart {
    path: PathBuf,
}

impl Autostart {
    /// Uses `$XDG_CONFIG_HOME/autostart`, or `~/.config/autostart`.
    pub fn for_user() -> Option<Self> {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(Self::in_config_home(&config_home))
    }

    pub fn in_config_home(config_home: &Path) -> Self {
        Self { path: config_home.join("autostart").join(format!("{APP_ID}.desktop")) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// True when the file exists and does not have `Hidden=true`.
    pub fn is_enabled(&self) -> bool {
        match fs::read_to_string(&self.path) {
            Ok(text) => !text.lines().any(|line| {
                line.split_once('=')
                    .is_some_and(|(key, value)| key.trim() == "Hidden" && value.trim() == "true")
            }),
            Err(_) => false,
        }
    }

    pub fn enable(&self, exec: &Path, args: &[&str]) -> io::Result<()> {
        let mut command = quote_exec_arg(&exec.to_string_lossy());
        for arg in args {
            command.push(' ');
            command.push_str(&quote_exec_arg(arg));
        }
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=Insert\nExec={command}\nIcon={APP_ID}\nX-GNOME-Autostart-enabled=true\n"
        );
        let dir = self.path.parent().expect("autostart path has a parent");
        fs::create_dir_all(dir)?;
        let temp = self.path.with_extension("desktop.tmp");
        fs::write(&temp, entry)?;
        fs::rename(&temp, &self.path)
    }

    pub fn disable(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

/// The program for the autostart entry: `~/.local/bin/insert` when it is an executable file (`install.sh`
/// puts Insert there), else the running program.
pub fn program() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    Ok(program_for(home.as_deref(), std::env::current_exe()?))
}

fn program_for(home: Option<&Path>, current_exe: PathBuf) -> PathBuf {
    if let Some(installed) = home.map(|home| home.join(".local/bin/insert")).filter(|path| is_executable(path)) {
        return installed;
    }
    // After the file is replaced, for example by a new install, Linux adds this suffix to the path of the
    // running program.
    match current_exe.to_str().and_then(|path| path.strip_suffix(" (deleted)")) {
        Some(path) => PathBuf::from(path),
        None => current_exe,
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Quotes one argument for the `Exec` key of the Desktop Entry Specification.
fn quote_exec_arg(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for character in arg.chars() {
        match character {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(character);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("insert-autostart-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn enable_then_disable_changes_the_state() {
        let home = temp_config_home("toggle");
        let autostart = Autostart::in_config_home(&home);
        assert!(!autostart.is_enabled());

        autostart.enable(Path::new("/opt/My Apps/insert"), &["--background"]).unwrap();
        assert!(autostart.is_enabled());
        let text = fs::read_to_string(autostart.path()).unwrap();
        assert!(text.contains("Exec=\"/opt/My Apps/insert\" \"--background\"\n"), "{text}");
        assert_eq!(autostart.path(), home.join("autostart/io.github.kenwuqianghao.Insert.desktop"));

        autostart.disable().unwrap();
        assert!(!autostart.is_enabled());
        autostart.disable().unwrap();
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_program_is_the_installed_binary_else_the_running_one_without_the_deleted_suffix() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = temp_config_home("program");
        let running = PathBuf::from("/opt/insert/insert (deleted)");
        assert_eq!(program_for(Some(&home), running.clone()), Path::new("/opt/insert/insert"));
        assert_eq!(program_for(None, PathBuf::from("/opt/insert/insert")), Path::new("/opt/insert/insert"));

        let installed = home.join(".local/bin/insert");
        fs::create_dir_all(installed.parent().unwrap()).unwrap();
        fs::write(&installed, "").unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(program_for(Some(&home), running.clone()), Path::new("/opt/insert/insert"));
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(program_for(Some(&home), running), installed);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_hidden_entry_is_not_enabled() {
        let home = temp_config_home("hidden");
        let autostart = Autostart::in_config_home(&home);
        autostart.enable(Path::new("/usr/bin/insert"), &[]).unwrap();
        let mut text = fs::read_to_string(autostart.path()).unwrap();
        text.push_str("Hidden=true\n");
        fs::write(autostart.path(), text).unwrap();
        assert!(!autostart.is_enabled());
        fs::remove_dir_all(&home).unwrap();
    }
}
