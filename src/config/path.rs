use std::path::PathBuf;

use directories::ProjectDirs;

pub static CONFIG_FILE_NAME: &str = "sattelclub.toml";

pub fn get_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("de", "jpoep", "sattelclub")
}

pub fn get_config_dirs() -> Vec<PathBuf> {
    let config_dir = get_project_dirs().map(|proj_dirs| proj_dirs.config_dir().to_path_buf());
    let current_dir = std::env::current_dir().ok();
    [current_dir, config_dir].into_iter().flatten().collect()
}

pub fn get_first_config_dir() -> PathBuf {
    get_config_dirs().into_iter().next().expect(
        "No valid config dir found, current directory nor system config directory are writable.",
    )
}

/// Finds the path to the configuration file by searching in standard locations.
///
/// The search order is:
/// 1. The platform-specific config directory (e.g., ~/.config/sattelclub/).
/// 2. The current working directory.
///
/// Returns the path if the file is found in any of these locations.
pub fn find_config_path() -> Option<PathBuf> {
    // Search for the config file in the collected paths.
    for mut path in get_config_dirs() {
        path.push(CONFIG_FILE_NAME);
        if path.exists() {
            println!("Found configuration file at: {}", path.display());
            return Some(path);
        }
    }
    None
}
