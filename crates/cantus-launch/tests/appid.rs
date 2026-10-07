//! The program's taskbar identity: set, and read back from Windows.

#[test]
fn the_process_takes_cantuss_app_id() {
    assert_eq!(cantus_launch::APP_ID, "Cantus.Cantus");
    assert_eq!(cantus_launch::set_app_id(), Ok(()));
    if cfg!(windows) {
        assert_eq!(
            cantus_launch::current_app_id().as_deref(),
            Some(cantus_launch::APP_ID)
        );
    }
}
