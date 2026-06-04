use super::*;

#[test]
fn read_config_t() {
    assert!(read_config(None).is_ok());
}
