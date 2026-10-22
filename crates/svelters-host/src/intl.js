// IN1: `Intl` for QuickJS-NG (which has none), over svelters-intl (ICU4X) natives `n`.
// Constructors validate options as ECMA-402 does where it is cheap; formatting runs in Rust.
// The default locale is the host's (`Host::set_locale`); `__svelters_intl` reports its changes
// and is read-only from JS.
// Deviations are listed in plans/milestones/IN1-intl.md.
(function (n, global) {
  'use strict';

  let current = n.current();
  const listeners = new Set();
  const define = (o, k, v) => Object.defineProperty(o, k, { value: v, writable: true, configurable: true, enumerable: false });
  const tagOf = Symbol('svelters.intl.tag');

  // ---- locales and options ----

  const localeList = (locales) => {
    if (locales === undefined) return [];
    if (typeof locales === 'string' || (locales !== null && typeof locales === 'object' && locales[tagOf])) locales = [locales];
    if (locales === null) throw new TypeError('Cannot convert null to object');
    const out = [];
    for (const t of Array.from(Object(locales))) {
      if (typeof t !== 'string' && (t === null || typeof t !== 'object')) throw new TypeError('Language ID should be string or object.');
      const c = n.canonicalize(t && typeof t === 'object' && t[tagOf] ? t[tagOf] : String(t));
      if (!out.includes(c)) out.push(c);
    }
    return out;
  };
  const resolveLocale = (locales) => n.resolve(localeList(locales));
  const toOptions = (options) => {
    if (options === undefined) return Object.create(null);
    if (options === null) throw new TypeError('Cannot convert null to object');
    return Object(options);
  };
  const str = (o, key, allowed, fallback, ctor) => {
    let v = o[key];
    if (v === undefined) return fallback;
    v = String(v);
    if (allowed && !allowed.includes(v)) throw new RangeError(`Value ${v} out of range for ${ctor} options property ${key}`);
    return v;
  };
  const bool = (o, key, fallback) => (o[key] === undefined ? fallback : Boolean(o[key]));
  const int = (o, key, min, max, fallback) => {
    let v = o[key];
    if (v === undefined) return fallback;
    v = Number(v);
    if (!Number.isFinite(v) || v < min || v > max) throw new RangeError(`${key} value is out of range.`);
    return Math.floor(v);
  };
  const supportedLocalesOf = (locales, options) => {
    if (options !== undefined) str(toOptions(options), 'localeMatcher', ['lookup', 'best fit'], 'best fit', 'supportedLocalesOf');
    return localeList(locales).filter((t) => n.supported(t));
  };
  const toNumber = (x) => (typeof x === 'bigint' ? Number(x) : Number(x));

  // ---- NumberFormat ----

  class NumberFormat {
    #locale;
    #o;
    #json;
    #format;
    constructor(locales, options) {
      const o = toOptions(options);
      const C = 'Intl.NumberFormat';
      this.#locale = resolveLocale(locales);
      str(o, 'localeMatcher', ['lookup', 'best fit'], 'best fit', C);
      str(o, 'numberingSystem', null, undefined, C);
      const r = {};
      r.style = str(o, 'style', ['decimal', 'percent', 'currency', 'unit'], 'decimal', C);
      if (r.style === 'unit') throw new RangeError('Intl.NumberFormat: style "unit" is not supported');
      const currency = str(o, 'currency', null, undefined, C);
      if (currency !== undefined && !/^[A-Za-z]{3}$/.test(currency)) throw new RangeError(`Invalid currency code : ${currency}`);
      const currencyDisplay = str(o, 'currencyDisplay', ['code', 'symbol', 'narrowSymbol', 'name'], 'symbol', C);
      str(o, 'currencySign', ['standard', 'accounting'], 'standard', C);
      if (r.style === 'currency') {
        if (currency === undefined) throw new TypeError('Currency code is required with currency style.');
        r.currency = currency.toUpperCase();
        r.currencyDisplay = currencyDisplay;
      }
      let notation = str(o, 'notation', ['standard', 'scientific', 'engineering', 'compact'], 'standard', C);
      // scientific and engineering format as standard (a documented deviation)
      if (notation !== 'compact') notation = 'standard';
      r.minimumIntegerDigits = int(o, 'minimumIntegerDigits', 1, 21, 1);
      const minFD = int(o, 'minimumFractionDigits', 0, 100, undefined);
      const maxFD = int(o, 'maximumFractionDigits', 0, 100, undefined);
      const minSD = int(o, 'minimumSignificantDigits', 1, 21, undefined);
      const maxSD = int(o, 'maximumSignificantDigits', 1, 21, undefined);
      if (minSD !== undefined || maxSD !== undefined) {
        r.minimumSignificantDigits = minSD ?? 1;
        r.maximumSignificantDigits = maxSD ?? 21;
        if (r.minimumSignificantDigits > r.maximumSignificantDigits) throw new RangeError('maximumSignificantDigits value is out of range.');
      } else {
        if (minFD !== undefined) r.minimumFractionDigits = minFD;
        if (maxFD !== undefined) r.maximumFractionDigits = maxFD;
      }
      r.notation = notation;
      if (notation === 'compact') r.compactDisplay = str(o, 'compactDisplay', ['short', 'long'], 'short', C);
      const g = o.useGrouping;
      if (g === undefined) r.useGrouping = notation === 'compact' ? 'min2' : 'auto';
      else if (g === true || g === 'true' || g === 'false') r.useGrouping = 'always';
      else if (g === false || g === null || g === '' || g === 0) r.useGrouping = false;
      else r.useGrouping = str(o, 'useGrouping', ['min2', 'auto', 'always'], 'auto', C);
      r.signDisplay = str(o, 'signDisplay', ['auto', 'never', 'always', 'exceptZero', 'negative'], 'auto', C);
      this.#json = JSON.stringify(r);
      if (r.minimumSignificantDigits === undefined) {
        const [min, max] = n.numberDigits(this.#json);
        r.minimumFractionDigits = min;
        r.maximumFractionDigits = max;
      }
      this.#o = r;
    }
    get format() {
      if (!this.#format) this.#format = (x) => n.number(this.#locale, this.#json, toNumber(x), false);
      return this.#format;
    }
    formatToParts(x) {
      return n.number(this.#locale, this.#json, toNumber(x), true);
    }
    resolvedOptions() {
      const r = this.#o;
      const out = { locale: this.#locale, numberingSystem: 'latn', style: r.style };
      if (r.style === 'currency') {
        out.currency = r.currency;
        out.currencyDisplay = r.currencyDisplay;
        out.currencySign = 'standard';
      }
      out.minimumIntegerDigits = r.minimumIntegerDigits;
      if (r.minimumSignificantDigits !== undefined) {
        out.minimumSignificantDigits = r.minimumSignificantDigits;
        out.maximumSignificantDigits = r.maximumSignificantDigits;
      } else {
        out.minimumFractionDigits = r.minimumFractionDigits;
        out.maximumFractionDigits = r.maximumFractionDigits;
      }
      out.useGrouping = r.useGrouping;
      out.notation = r.notation;
      if (r.compactDisplay) out.compactDisplay = r.compactDisplay;
      out.signDisplay = r.signDisplay;
      out.roundingIncrement = 1;
      out.roundingMode = 'halfExpand';
      out.roundingPriority = 'auto';
      out.trailingZeroDisplay = 'auto';
      return out;
    }
    static supportedLocalesOf(locales, options) {
      return supportedLocalesOf(locales, options);
    }
  }
  define(NumberFormat.prototype, Symbol.toStringTag, 'Intl.NumberFormat');

  // ---- PluralRules ----

  class PluralRules {
    #locale;
    #ordinal;
    #min;
    #max;
    constructor(locales, options) {
      if (new.target === undefined) throw new TypeError("Constructor Intl.PluralRules requires 'new'");
      const o = toOptions(options);
      const C = 'Intl.PluralRules';
      this.#locale = resolveLocale(locales);
      str(o, 'localeMatcher', ['lookup', 'best fit'], 'best fit', C);
      this.#ordinal = str(o, 'type', ['cardinal', 'ordinal'], 'cardinal', C) === 'ordinal';
      const minFD = int(o, 'minimumFractionDigits', 0, 100, undefined);
      const maxFD = int(o, 'maximumFractionDigits', 0, 100, undefined);
      this.#min = minFD ?? 0;
      this.#max = maxFD ?? Math.max(this.#min, 3);
      if (this.#min > this.#max) throw new RangeError('maximumFractionDigits value is out of range.');
    }
    select(x) {
      return n.plural(this.#locale, this.#ordinal, this.#min, this.#max, Number(x));
    }
    resolvedOptions() {
      return {
        locale: this.#locale,
        type: this.#ordinal ? 'ordinal' : 'cardinal',
        minimumIntegerDigits: 1,
        minimumFractionDigits: this.#min,
        maximumFractionDigits: this.#max,
        pluralCategories: n.pluralCategories(this.#locale, this.#ordinal),
        roundingIncrement: 1,
        roundingMode: 'halfExpand',
        roundingPriority: 'auto',
        trailingZeroDisplay: 'auto',
      };
    }
    static supportedLocalesOf(locales, options) {
      return supportedLocalesOf(locales, options);
    }
  }
  define(PluralRules.prototype, Symbol.toStringTag, 'Intl.PluralRules');

  // ---- Collator ----

  class Collator {
    #locale;
    #o;
    #json;
    #compare;
    constructor(locales, options) {
      const o = toOptions(options);
      const C = 'Intl.Collator';
      this.#locale = resolveLocale(locales);
      const usage = str(o, 'usage', ['sort', 'search'], 'sort', C);
      str(o, 'localeMatcher', ['lookup', 'best fit'], 'best fit', C);
      const r = { usage };
      r.sensitivity = str(o, 'sensitivity', ['base', 'accent', 'case', 'variant'], 'variant', C);
      r.ignorePunctuation = bool(o, 'ignorePunctuation', false);
      const kn = /-u(?:-[a-z0-9]{2,8})*?-kn(?:-([a-z0-9]+))?/.exec(this.#locale);
      r.numeric = bool(o, 'numeric', kn ? kn[1] === undefined || kn[1] === 'true' : false);
      r.caseFirst = str(o, 'caseFirst', ['upper', 'lower', 'false'], 'false', C);
      this.#o = r;
      this.#json = JSON.stringify(r);
    }
    get compare() {
      if (!this.#compare) this.#compare = (a, b) => n.compare(this.#locale, this.#json, String(a), String(b));
      return this.#compare;
    }
    resolvedOptions() {
      return { locale: this.#locale, usage: this.#o.usage, sensitivity: this.#o.sensitivity, ignorePunctuation: this.#o.ignorePunctuation, collation: 'default', numeric: this.#o.numeric, caseFirst: this.#o.caseFirst };
    }
    static supportedLocalesOf(locales, options) {
      return supportedLocalesOf(locales, options);
    }
  }
  define(Collator.prototype, Symbol.toStringTag, 'Intl.Collator');

  // ---- DateTimeFormat ----

  const DATE_FIELDS = ['weekday', 'era', 'year', 'month', 'day'];
  const TIME_FIELDS = ['dayPeriod', 'hour', 'minute', 'second', 'fractionalSecondDigits'];
  const UTC_NAMES = ['utc', 'etc/utc', 'gmt', 'etc/gmt', 'etc/universal', 'etc/zulu', 'universal', 'zulu', 'etc/gmt0', 'etc/gmt+0', 'etc/gmt-0'];
  const pad2 = (v) => String(v).padStart(2, '0');
  const offsetName = (secs) => {
    const s = secs < 0 ? '-' : '+';
    const a = Math.abs(secs);
    return `${s}${pad2(Math.floor(a / 3600))}:${pad2(Math.floor((a % 3600) / 60))}`;
  };
  /** -> { utc: bool, fixed: seconds | undefined, name } */
  const timeZone = (tz) => {
    if (tz === undefined) {
      const name = n.localTimeZone();
      return { utc: false, fixed: undefined, name: name || offsetName(-new Date().getTimezoneOffset() * 60) };
    }
    const s = String(tz);
    if (UTC_NAMES.includes(s.toLowerCase())) return { utc: true, fixed: 0, name: 'UTC' };
    const m = /^([+-])(\d{2})(?::?(\d{2}))?$/.exec(s);
    if (m) {
      const h = Number(m[2]);
      const mi = Number(m[3] ?? 0);
      if (h > 23 || mi > 59) throw new RangeError(`Invalid time zone specified: ${s}`);
      const secs = (m[1] === '-' ? -1 : 1) * (h * 3600 + mi * 60);
      return { utc: false, fixed: secs, name: offsetName(secs) };
    }
    const local = n.localTimeZone();
    if (local && s.toLowerCase() === local.toLowerCase()) return { utc: false, fixed: undefined, name: local };
    throw new RangeError(`Invalid time zone specified: ${s} (svelters' Intl knows UTC, offsets and the host's zone)`);
  };

  class DateTimeFormat {
    #locale;
    #o;
    #json;
    #tz;
    #format;
    constructor(locales, options, required = 'any', defaults = 'date') {
      const o = toOptions(options);
      const C = 'Intl.DateTimeFormat';
      this.#locale = resolveLocale(locales);
      str(o, 'localeMatcher', ['lookup', 'best fit'], 'best fit', C);
      str(o, 'calendar', null, undefined, C);
      str(o, 'numberingSystem', null, undefined, C);
      const hour12 = o.hour12 === undefined ? undefined : Boolean(o.hour12);
      let hourCycle = str(o, 'hourCycle', ['h11', 'h12', 'h23', 'h24'], undefined, C);
      if (hour12 !== undefined) hourCycle = hour12 ? 'h12' : 'h23';
      this.#tz = timeZone(o.timeZone);
      const r = {};
      r.weekday = str(o, 'weekday', ['narrow', 'short', 'long'], undefined, C);
      r.era = str(o, 'era', ['narrow', 'short', 'long'], undefined, C);
      r.year = str(o, 'year', ['2-digit', 'numeric'], undefined, C);
      r.month = str(o, 'month', ['2-digit', 'numeric', 'narrow', 'short', 'long'], undefined, C);
      r.day = str(o, 'day', ['2-digit', 'numeric'], undefined, C);
      r.dayPeriod = str(o, 'dayPeriod', ['narrow', 'short', 'long'], undefined, C);
      r.hour = str(o, 'hour', ['2-digit', 'numeric'], undefined, C);
      r.minute = str(o, 'minute', ['2-digit', 'numeric'], undefined, C);
      r.second = str(o, 'second', ['2-digit', 'numeric'], undefined, C);
      r.fractionalSecondDigits = int(o, 'fractionalSecondDigits', 1, 3, undefined);
      r.timeZoneName = str(o, 'timeZoneName', ['short', 'long', 'shortOffset', 'longOffset', 'shortGeneric', 'longGeneric'], undefined, C);
      str(o, 'formatMatcher', ['basic', 'best fit'], 'best fit', C);
      r.dateStyle = str(o, 'dateStyle', ['full', 'long', 'medium', 'short'], undefined, C);
      r.timeStyle = str(o, 'timeStyle', ['full', 'long', 'medium', 'short'], undefined, C);
      const anyDate = DATE_FIELDS.some((k) => r[k] !== undefined);
      const anyTime = TIME_FIELDS.some((k) => r[k] !== undefined);
      if (r.dateStyle || r.timeStyle) {
        if (anyDate || anyTime || r.timeZoneName) throw new TypeError(`Can't set option ${anyDate ? DATE_FIELDS.find((k) => r[k] !== undefined) : 'timeZoneName'} when dateStyle or timeStyle is used`);
        if (required === 'date' && r.timeStyle) throw new TypeError('Invalid option : timeStyle');
        if (required === 'time' && r.dateStyle) throw new TypeError('Invalid option : dateStyle');
      } else {
        let needDefaults = true;
        if ((required === 'date' || required === 'any') && anyDate) needDefaults = false;
        if ((required === 'time' || required === 'any') && anyTime) needDefaults = false;
        if (needDefaults && (defaults === 'date' || defaults === 'all')) r.year = r.month = r.day = 'numeric';
        if (needDefaults && (defaults === 'time' || defaults === 'all')) r.hour = r.minute = r.second = 'numeric';
      }
      if (hourCycle !== undefined && (r.hour !== undefined || r.timeStyle !== undefined)) r.hourCycle = hourCycle;
      for (const k of Object.keys(r)) if (r[k] === undefined) delete r[k];
      this.#o = r;
      this.#json = JSON.stringify(r);
    }
    #run(date, parts) {
      const x = date === undefined ? Date.now() : Number(date instanceof Date ? date.getTime() : date);
      if (!Number.isFinite(x)) throw new RangeError('Invalid time value');
      const tz = this.#tz;
      const offset = tz.utc ? 0 : tz.fixed !== undefined ? tz.fixed : -new Date(x).getTimezoneOffset() * 60;
      return n.date(this.#locale, this.#json, x, tz.utc, offset, parts);
    }
    get format() {
      if (!this.#format) this.#format = (date) => this.#run(date, false);
      return this.#format;
    }
    formatToParts(date) {
      return this.#run(date, true);
    }
    resolvedOptions() {
      const r = this.#o;
      const out = { locale: this.#locale, calendar: 'gregory', numberingSystem: 'latn', timeZone: this.#tz.name };
      if (r.hourCycle) {
        out.hourCycle = r.hourCycle === 'h24' ? 'h23' : r.hourCycle;
        out.hour12 = out.hourCycle === 'h11' || out.hourCycle === 'h12';
      }
      for (const k of [...DATE_FIELDS, ...TIME_FIELDS, 'timeZoneName', 'dateStyle', 'timeStyle']) if (r[k] !== undefined) out[k] = r[k];
      return out;
    }
    static supportedLocalesOf(locales, options) {
      return supportedLocalesOf(locales, options);
    }
  }
  define(DateTimeFormat.prototype, Symbol.toStringTag, 'Intl.DateTimeFormat');

  // ---- Intl ----

  // NumberFormat, Collator and DateTimeFormat may be called without `new` (ECMA-402).
  const callable = (C, name) => {
    const f = function (locales, options) {
      return new C(locales, options);
    };
    Object.defineProperty(f, 'name', { value: name });
    f.prototype = C.prototype;
    define(C.prototype, 'constructor', f);
    define(f, 'supportedLocalesOf', supportedLocalesOf);
    return f;
  };
  const Intl = {};
  define(Intl, Symbol.toStringTag, 'Intl');
  define(Intl, 'NumberFormat', callable(NumberFormat, 'NumberFormat'));
  define(Intl, 'PluralRules', PluralRules);
  define(Intl, 'Collator', callable(Collator, 'Collator'));
  define(Intl, 'DateTimeFormat', callable(DateTimeFormat, 'DateTimeFormat'));
  define(Intl, 'getCanonicalLocales', (locales) => localeList(locales));
  define(global, 'Intl', Intl);

  // ---- the locale-sensitive built-ins ----

  // Default formatters, rebuilt after a locale change.
  let defaults = {};
  const cached = (key, make) => defaults[key] || (defaults[key] = make());
  const isDefault = (locales, options) => locales === undefined && options === undefined;

  define(Number.prototype, 'toLocaleString', function toLocaleString(locales, options) {
    const x = Number.prototype.valueOf.call(this);
    const nf = isDefault(locales, options) ? cached('nf', () => new NumberFormat()) : new NumberFormat(locales, options);
    return nf.format(x);
  });
  if (typeof BigInt === 'function') {
    define(BigInt.prototype, 'toLocaleString', function toLocaleString(locales, options) {
      const x = BigInt.prototype.valueOf.call(this);
      const nf = isDefault(locales, options) ? cached('nf', () => new NumberFormat()) : new NumberFormat(locales, options);
      return nf.format(x);
    });
  }
  const dateMethod = (name, required, defs) =>
    define(Date.prototype, name, {
      [name](locales, options) {
        const t = Date.prototype.getTime.call(this);
        if (Number.isNaN(t)) return 'Invalid Date';
        const f = isDefault(locales, options) ? cached(name, () => new DateTimeFormat(undefined, undefined, required, defs)) : new DateTimeFormat(locales, options, required, defs);
        return f.format(t);
      },
    }[name]);
  dateMethod('toLocaleString', 'any', 'all');
  dateMethod('toLocaleDateString', 'date', 'date');
  dateMethod('toLocaleTimeString', 'time', 'time');
  define(String.prototype, 'localeCompare', function localeCompare(that, locales, options) {
    if (this === undefined || this === null) throw new TypeError('String.prototype.localeCompare called on null or undefined');
    const c = isDefault(locales, options) ? cached('collator', () => new Collator()) : new Collator(locales, options);
    return c.compare(String(this), String(that));
  });

  // ---- the host's locale ----

  // `__svelters_intl` (used by `svelters:intl`): the host's current locale and a store-contract
  // `subscribe`, so components re-render when the host switches language. Read-only from JS
  // (frozen, non-writable): only Rust's `Host::set_locale` changes the locale, through the
  // `changed` function this shim returns to Rust and never exposes to JS.
  const api = Object.freeze({
    get locale() {
      return current;
    },
    subscribe(run) {
      if (typeof run !== 'function') throw new TypeError('subscribe: callback must be a function');
      run(current);
      const entry = { run };
      listeners.add(entry);
      return () => listeners.delete(entry);
    },
  });
  Object.defineProperty(global, '__svelters_intl', { value: api, writable: false, configurable: false, enumerable: false });

  return function changed(tag) {
    current = tag;
    defaults = {};
    let first;
    for (const l of [...listeners]) {
      try {
        l.run(tag);
      } catch (e) {
        if (first === undefined) first = e;
        else global.console.error(e);
      }
    }
    if (first !== undefined) throw first;
  };
});
