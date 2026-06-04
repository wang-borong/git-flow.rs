use std::str::FromStr;

use super::*;

#[test]
fn get_global_config_path_t() {
    if env::consts::OS == "linux" {
        let global_config_path = get_global_config_path().unwrap();
        let home = env::var("HOME").unwrap();
        let expected = PathBuf::from(&home).join(".config/git-flow/config.toml");
        assert_eq!(global_config_path, expected);
    }
}

#[test]
fn get_local_config_path_t() {
    let local_config_path = get_local_config_path().unwrap();
    let mut path = PathBuf::from_str(local_config_path.to_str().unwrap()).unwrap();
    path.pop();
    assert!(path.join(".git").is_dir());
}
