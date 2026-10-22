// IN1: Intl in QuickJS through the host (crates/svelters-host/tests/intl.rs). Prints one line per
// check: "ok <name>" or "FAIL <name>: <got> !== <want>".
const eq = (name, got, want) => {
  const g = JSON.stringify(got);
  const w = JSON.stringify(want);
  console.log(g === w ? `ok ${name}` : `FAIL ${name}: ${g} !== ${w}`);
};
const throws = (name, f, ctor) => {
  try {
    f();
    console.log(`FAIL ${name}: no throw`);
  } catch (e) {
    console.log(e instanceof ctor ? `ok ${name}` : `FAIL ${name}: threw ${e && e.name}: ${e && e.message}`);
  }
};

// NumberFormat
eq('nf de', new Intl.NumberFormat('de-DE').format(1234.5), '1.234,5');
eq('nf en', new Intl.NumberFormat('en-US').format(1234.5), '1,234.5');
eq('nf no new', Intl.NumberFormat('de').format(0.5), '0,5');
eq('nf currency', new Intl.NumberFormat('de-DE', { style: 'currency', currency: 'EUR' }).format(1234.5), '1.234,50 €');
eq('nf percent', new Intl.NumberFormat('en', { style: 'percent' }).format(0.256), '26%');
eq('nf digits', new Intl.NumberFormat('en', { minimumFractionDigits: 2 }).format(1), '1.00');
eq('nf grouping', new Intl.NumberFormat('en', { useGrouping: false }).format(1234567), '1234567');
eq('nf sign', new Intl.NumberFormat('en', { signDisplay: 'always' }).format(5), '+5');
eq('nf compact', new Intl.NumberFormat('en', { notation: 'compact' }).format(1234), '1.2K');
eq('nf parts', new Intl.NumberFormat('de').formatToParts(-1234.5).map((p) => p.type), ['minusSign', 'integer', 'group', 'integer', 'decimal', 'fraction']);
eq('nf resolved', (({ locale, minimumFractionDigits, maximumFractionDigits, useGrouping }) => ({ locale, minimumFractionDigits, maximumFractionDigits, useGrouping }))(new Intl.NumberFormat('de-DE').resolvedOptions()), { locale: 'de-DE', minimumFractionDigits: 0, maximumFractionDigits: 3, useGrouping: 'auto' });
eq('nf currency digits', new Intl.NumberFormat('ja', { style: 'currency', currency: 'JPY' }).resolvedOptions().maximumFractionDigits, 0);
eq('nf unsupported falls back', new Intl.NumberFormat('sw').resolvedOptions().locale, 'en-US');
throws('nf bad style', () => new Intl.NumberFormat('en', { style: 'bogus' }), RangeError);
throws('nf currency missing', () => new Intl.NumberFormat('en', { style: 'currency' }), TypeError);
throws('nf bad tag', () => new Intl.NumberFormat('not a tag'), RangeError);
throws('nf bad digits', () => new Intl.NumberFormat('en', { maximumFractionDigits: 101 }), RangeError);
eq('toLocaleString default', (1234.5).toLocaleString(), '1,234.5');
eq('toLocaleString de', (1234.5).toLocaleString('de-DE'), '1.234,5');
eq('toLocaleString options', (0.5).toLocaleString('en', { style: 'percent' }), '50%');

// PluralRules
const cats = (tag, ns) => ns.map((n) => new Intl.PluralRules(tag).select(n));
eq('pr en', cats('en', [0, 1, 2]), ['other', 'one', 'other']);
eq('pr pl', cats('pl', [1, 2, 5, 22, 25, 1.5]), ['one', 'few', 'many', 'few', 'many', 'other']);
eq('pr ru', cats('ru', [1, 3, 5, 11, 21]), ['one', 'few', 'many', 'many', 'one']);
eq('pr ordinal', [1, 2, 3, 4, 11, 22].map((n) => new Intl.PluralRules('en', { type: 'ordinal' }).select(n)), ['one', 'two', 'few', 'other', 'other', 'two']);
eq('pr categories', new Intl.PluralRules('pl').resolvedOptions().pluralCategories, ['one', 'few', 'many', 'other']);
throws('pr needs new', () => Intl.PluralRules('en'), TypeError);

// DateTimeFormat (UTC: the test must not depend on the machine's zone)
const t = Date.UTC(2024, 2, 5, 14, 7, 9);
eq('dtf en long', new Intl.DateTimeFormat('en-US', { dateStyle: 'long', timeZone: 'UTC' }).format(t), 'March 5, 2024');
eq('dtf de long', new Intl.DateTimeFormat('de-DE', { dateStyle: 'long', timeZone: 'UTC' }).format(new Date(t)), '5. März 2024');
eq('dtf fr full', new Intl.DateTimeFormat('fr', { dateStyle: 'full', timeZone: 'UTC' }).format(t), 'mardi 5 mars 2024');
eq('dtf components', new Intl.DateTimeFormat('en-US', { weekday: 'long', month: 'long', day: 'numeric', timeZone: 'UTC' }).format(t), 'Tuesday, March 5');
eq('dtf time', new Intl.DateTimeFormat('de-DE', { timeStyle: 'short', timeZone: 'UTC' }).format(t), '14:07');
eq('dtf hour12', new Intl.DateTimeFormat('en-US', { hour: 'numeric', minute: '2-digit', hour12: false, timeZone: 'UTC' }).format(t), '14:07');
eq('dtf offset zone', new Intl.DateTimeFormat('en-US', { hour: 'numeric', minute: '2-digit', hourCycle: 'h23', timeZone: '+02:00', timeZoneName: 'shortOffset' }).format(t), '16:07 GMT+2');
eq('dtf parts', new Intl.DateTimeFormat('de-DE', { dateStyle: 'medium', timeZone: 'UTC' }).formatToParts(t).map((p) => p.type), ['day', 'literal', 'month', 'literal', 'year']);
eq('dtf resolved', new Intl.DateTimeFormat('en', { timeZone: 'utc' }).resolvedOptions().timeZone, 'UTC');
throws('dtf bad zone', () => new Intl.DateTimeFormat('en', { timeZone: 'Mars/Olympus' }), RangeError);
throws('dtf invalid date', () => new Intl.DateTimeFormat('en').format(NaN), RangeError);
throws('dtf style and component', () => new Intl.DateTimeFormat('en', { dateStyle: 'long', year: 'numeric' }), TypeError);
// V8 prints 5.3.2024 (CLDR skeleton yMd); ICU4X's numeric date field set pads (a documented deviation)
eq('toLocaleDateString', new Date(t).toLocaleDateString('de-DE', { timeZone: 'UTC' }), '05.03.2024');
eq('toLocaleTimeString', new Date(t).toLocaleTimeString('de-DE', { timeZone: 'UTC' }), '14:07:09');
eq('toLocaleString date', new Date(t).toLocaleString('en-US', { timeZone: 'UTC' }), '3/5/2024, 2:07:09 PM');
eq('invalid date', new Date(NaN).toLocaleString(), 'Invalid Date');

// Collator and localeCompare
eq('collator de', ['Zebra', 'Äpfel', 'Apfel', 'Bär'].sort(new Intl.Collator('de').compare), ['Apfel', 'Äpfel', 'Bär', 'Zebra']);
eq('localeCompare de', ['zahl', 'ähnlich', 'aber'].sort((a, b) => a.localeCompare(b, 'de')), ['aber', 'ähnlich', 'zahl']);
eq('localeCompare base', 'a'.localeCompare('Ä', 'de', { sensitivity: 'base' }), 0);
eq('collator numeric', new Intl.Collator('en', { numeric: true }).compare('item 10', 'item 9'), 1);
eq('collator -u-kn', new Intl.Collator('en-u-kn').resolvedOptions().numeric, true);

// locales
eq('canonical', Intl.getCanonicalLocales(['EN-us', 'de-de', 'en-US']), ['en-US', 'de-DE']);
eq('supportedLocalesOf', Intl.NumberFormat.supportedLocalesOf(['sw', 'de-AT', 'ja']), ['de-AT', 'ja']);
eq('toStringTag', Object.prototype.toString.call(new Intl.NumberFormat()), '[object Intl.NumberFormat]');

// the host's locale: a store-contract subscribe, seen by Rust's set_locale (tests/intl.rs)
eq('host locale', __svelters_intl.locale, 'en-US');
globalThis.seen = [];
__svelters_intl.subscribe((tag) => {
  seen.push(`${tag} ${(1234.5).toLocaleString()} ${new Intl.PluralRules().select(2)}`);
  console.log(`locale ${seen[seen.length - 1]}`);
});
