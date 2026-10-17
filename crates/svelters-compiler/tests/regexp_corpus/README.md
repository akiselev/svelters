# Regexp validation corpus (acorn 8.19.0 oracle)

`corpus.json` holds regular-expression literals with the verdict of real acorn 8.19.0 for each
`ecmaVersion` in `versions` (5, 6, 9, 10, 11, 12, 13, 14, 15, 16: every version where
`src/regexp.js` or `src/unicode-property-data.js` changes behaviour). `../regexp_corpus.rs` runs
`acorn::regexp::validate_flags` + `validate_pattern` over it and requires exact agreement
(message text included). Do not edit `corpus.json` by hand; regenerate it.

## Regenerating

```sh
node crates/svelters-compiler/tests/regexp_corpus/generate.cjs \
  node_modules/.pnpm/acorn@8.19.0/node_modules/acorn \
  <acorn repo checkout at tag 8.19.0>/test
```

The script

1. evaluates every `test/tests*.js` file of the acorn repo with `test`/`testFail`/`testAssert`
   stubbed, parses each collected input with acorn while `Parser.prototype.validateRegExpFlags`
   is hooked, and keeps every `(pattern, flags)` acorn validated;
2. adds hand-written edge cases (named groups and duplicates, modifiers, `u`/`v` escapes,
   surrogates, unicode properties per ecmaVersion, class set operations, huge numbers) and
   ~9000 deterministic random token combinations;
3. for each pair and version runs `acorn.parse("/" + pattern + "/" + flags, {ecmaVersion})`. A
   verdict is "accepted" (0) if pattern validation completed, otherwise the regexp error message
   (acorn raises all of them at position 1, the pattern start). Pairs whose source does not form
   exactly that one regex literal, contain lone surrogates (not representable in a Rust `&str`)
   or fail with a non-regexp error are dropped.

Case format: `[pattern, flags, result | [result per version], origin]`; result `0` = accepted,
`":msg"` = `Invalid regular expression: /<pattern>/: msg`, any other string = the flag error.
