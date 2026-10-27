//! `skelter-gallery-app [<story>[/<variant>]]`: svelters' own gallery (`gallery/`).
fn main() -> std::process::ExitCode {
    skelter_gallery_app::run(skelter_gallery::Extensions::default())
}
