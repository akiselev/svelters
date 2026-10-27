//! svelters' own gallery: the stories under `gallery/` (found from the current directory, or
//! `--root`), the built-in kinds and the four VS Code themes. See `docs/skelter/gallery.md`.

fn main() -> std::process::ExitCode {
    skelter_gallery::cli_main(skelter_gallery::Extensions::new())
}
