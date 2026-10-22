//! The ICU4X data: one postcard blob, deserialized lazily, behind a locale fallback provider.

use crate::IntlError;
use icu_locale::fallback::LocaleFallbacker;
use icu_provider::buf::AsDeserializingBufferProvider;
use icu_provider_adapters::fallback::LocaleFallbackProvider;
use icu_provider_blob::BlobDataProvider;
use std::sync::OnceLock;

/// The built-in blob (`tools/intl/gen-data.sh`) and the locales it was made for.
static BUILTIN_BLOB: &[u8] = include_bytes!("../data/icu4x.postcard");
const BUILTIN_LOCALES: &str = include_str!("../data/locales.txt");

pub(crate) type Provider = LocaleFallbackProvider<BlobDataProvider>;

pub(crate) struct Data {
    pub provider: Provider,
    pub locales: Vec<String>,
}

static CHOSEN: OnceLock<(&'static [u8], Vec<String>)> = OnceLock::new();
static DATA: OnceLock<Data> = OnceLock::new();

/// Uses `blob` (an `icu4x-datagen --format blob` output with the markers of
/// `tools/intl/gen-data.sh`) for the locales `locales` instead of the built-in data. Call it once,
/// before anything formats; later calls fail.
pub fn use_data(blob: &'static [u8], locales: &[&str]) -> Result<(), IntlError> {
    if DATA.get().is_some() {
        return Err(IntlError::Data("use_data after the data was first used".into()));
    }
    let locales = locales.iter().map(|s| s.to_string()).collect();
    CHOSEN.set((blob, locales)).map_err(|_| IntlError::Data("use_data called twice".into()))
}

/// The locales the data has (the built-in list or the one given to [`use_data`]).
pub fn data_locales() -> &'static [String] {
    &data().locales
}

pub(crate) fn data() -> &'static Data {
    DATA.get_or_init(|| {
        let (blob, locales) = match CHOSEN.get() {
            Some((b, l)) => (*b, l.clone()),
            None => (BUILTIN_BLOB, BUILTIN_LOCALES.lines().map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(String::from).collect()),
        };
        // The built-in blob is checked by the tests; a broken embedder blob is a programming error.
        make(blob, locales).unwrap_or_else(|e| panic!("svelters-intl: invalid ICU4X data blob: {e}"))
    })
}

fn make(blob: &'static [u8], locales: Vec<String>) -> Result<Data, IntlError> {
    let blob = BlobDataProvider::try_new_from_static_blob(blob)?;
    let fallbacker = LocaleFallbacker::try_new_unstable(&blob.as_deserializing())?;
    Ok(Data { provider: LocaleFallbackProvider::new(blob, fallbacker), locales })
}
