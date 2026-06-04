use super::*;
use crate::config::read::read_config;

#[test]
fn validate_config_t() {
    let config_list = read_config(None).unwrap();
    assert!(validate_config(&config_list).is_ok());
}
