#!/usr/bin/env bash
# IN1: regenerates crates/svelters-intl/data/icu4x.postcard (ICU4X data for svelters' Intl and the
# numeric widgets) for a locale subset, from pinned CLDR/ICU sources.
#
#   tools/intl/gen-data.sh                       # the default subset below
#   LOCALES="en de fr ja" tools/intl/gen-data.sh # another subset
#   OUT=/tmp/my.postcard tools/intl/gen-data.sh  # an embedder's blob (svelters_intl::use_data)
#
# Needs icu4x-datagen 2.4.0 built with `unstable` (the experimental currency/percent markers):
#   cargo install icu4x-datagen --version 2.4.0 --locked --features unstable
# (DATAGEN=/path/to/icu4x-datagen to use another binary). The icu crates in
# crates/svelters-intl/Cargo.toml are pinned to the same 2.4.0: advance both together.
#
# The marker list is what svelters-intl's constructors load (found with `--markers-for-bin` on its
# test binary), Gregorian calendar only: svelters-intl always formats dates in the Gregorian
# calendar. Sources are pinned: CLDR 49.0.0, icuexport release-79.1rc, tzdb 2026e (what datagen 2.4.0 calls latest).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
DATAGEN=${DATAGEN:-icu4x-datagen}
LOCALES=${LOCALES:-"en de fr es it ja zh ru pl"}
OUT=${OUT:-$root/crates/svelters-intl/data/icu4x.postcard}
CLDR_TAG=${CLDR_TAG:-49.0.0}
ICUEXPORT_TAG=${ICUEXPORT_TAG:-release-79.1rc}
TZDB_TAG=${TZDB_TAG:-2026e}

MARKERS=(
  CalendarPreferredV1
  CollationDiacriticsV1 CollationJamoV1 CollationMetadataV1 CollationReorderingV1 CollationRootV1
  CollationSpecialPrimariesV1 CollationTailoringV1 NormalizerNfdDataV1 NormalizerNfdTablesV1
  CurrencyDecimalSymbolsV1 CurrencyEssentialsV1 CurrencyExtendedDataV1 CurrencyFractionsV1
  CurrencyPatternsDataV1 CurrencyPatternsNoCurrencyV1 CurrencySymbolsV1
  DatetimeNamesDayperiodV1 DatetimeNamesMonthGregorianV1 DatetimeNamesWeekdayV1 DatetimeNamesYearGregorianV1
  DatetimePatternsDateGregorianV1 DatetimePatternsGlueV1 DatetimePatternsTimeV1
  DecimalCompactLongV1 DecimalCompactShortV1 DecimalDigitsV1 DecimalPercentV1 DecimalSymbolsV1
  LocaleLikelySubtagsLanguageV1 LocaleParentsV1
  PluralsCardinalV1 PluralsOrdinalV1
  TimezoneNamesEssentialsV1 TimezoneNamesSpecificLongV1 TimezoneNamesSpecificShortV1
  TimezoneNamesGenericLongV1 TimezoneNamesGenericShortV1 TimezoneNamesStandardLongV1
  TimezoneNamesLocationsOverrideV1 TimezoneNamesLocationsRootV1
  TimezoneNamesCitiesOverrideV1 TimezoneNamesCitiesRootV1 TimezonePeriodsV1
)

# shellcheck disable=SC2086
"$DATAGEN" --format blob --overwrite --out "$OUT" \
  --cldr-tag "$CLDR_TAG" --icuexport-tag "$ICUEXPORT_TAG" --tzdb-tag "$TZDB_TAG" \
  --markers "${MARKERS[@]}" --locales $LOCALES

if [ "$OUT" = "$root/crates/svelters-intl/data/icu4x.postcard" ]; then
  { echo "# The locales of data/icu4x.postcard (tools/intl/gen-data.sh)"; for l in $LOCALES; do echo "$l"; done; } \
    > "$root/crates/svelters-intl/data/locales.txt"
fi
ls -l "$OUT"
