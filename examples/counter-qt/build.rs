fn main() {
    svelters_qt_build::Config::new()
        .qt_from_pkg_config("Qt6Widgets")
        .profile_dev_fat()
        .with_verification_tests()
        .build();
}
