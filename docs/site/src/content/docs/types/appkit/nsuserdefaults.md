---
title: NSUserDefaults
description: "NSUserDefaults from svelters:appkit/foundation, a object class generated from objc2 facts."
---

```js
import { NSUserDefaults } from 'svelters:appkit/foundation';
```

## Class

`NSUserDefaults` (Foundation), category **object**.

Lifetime: **SHARED**: one JS object per native object, holding one strong reference (retain); `dispose()` or garbage collection drops our reference only.

Facts: `metadata/appkit/types/Foundation.json`; ledger fact ids `svelters-api:v1:type:appkit:Foundation:NSUserDefaults:*` in `metadata/verified/appkit.json`. Methods are called by selector; the JS name joins the selector's pieces.

## Construct

- `new NSUserDefaults(props?)`: `[[NSUserDefaults alloc] init]`, then the properties by KVC
- `NSUserDefaults.newWithSuiteName(suitename)`: `-[NSUserDefaults initWithSuiteName:]`

## Statics

- `NSUserDefaults.resetStandardUserDefaults()`: `+resetStandardUserDefaults`
- `NSUserDefaults.standardUserDefaults()`: `+standardUserDefaults`

## Methods

- `URLForKey(defaultName)`: `-URLForKey:`
- `addSuiteNamed(suiteName)`: `-addSuiteNamed:`
- `arrayForKey(defaultName)`: `-arrayForKey:`
- `boolForKey(defaultName)`: `-boolForKey:`
- `dataForKey(defaultName)`: `-dataForKey:`
- `dictionaryForKey(defaultName)`: `-dictionaryForKey:`
- `dictionaryRepresentation()`: `-dictionaryRepresentation`
- `doubleForKey(defaultName)`: `-doubleForKey:`
- `floatForKey(defaultName)`: `-floatForKey:`
- `integerForKey(defaultName)`: `-integerForKey:`
- `objectForKey(defaultName)`: `-objectForKey:`
- `objectIsForcedForKey(key)`: `-objectIsForcedForKey:`
- `objectIsForcedForKeyInDomain(key, domain)`: `-objectIsForcedForKey:inDomain:`
- `persistentDomainForName(domainName)`: `-persistentDomainForName:`
- `registerDefaults(registrationDictionary)`: `-registerDefaults:`
- `removeObjectForKey(defaultName)`: `-removeObjectForKey:`
- `removePersistentDomainForName(domainName)`: `-removePersistentDomainForName:`
- `removeSuiteNamed(suiteName)`: `-removeSuiteNamed:`
- `removeVolatileDomainForName(domainName)`: `-removeVolatileDomainForName:`
- `setBoolForKey(value, defaultName)`: `-setBool:forKey:`
- `setDoubleForKey(value, defaultName)`: `-setDouble:forKey:`
- `setFloatForKey(value, defaultName)`: `-setFloat:forKey:`
- `setIntegerForKey(value, defaultName)`: `-setInteger:forKey:`
- `setObjectForKey(value, defaultName)`: `-setObject:forKey:`
- `setPersistentDomainForName(domain, domainName)`: `-setPersistentDomain:forName:`
- `setURLForKey(url, defaultName)`: `-setURL:forKey:`
- `setVolatileDomainForName(domain, domainName)`: `-setVolatileDomain:forName:`
- `stringArrayForKey(defaultName)`: `-stringArrayForKey:`
- `stringForKey(defaultName)`: `-stringForKey:`
- `synchronize()`: `-synchronize`
- `volatileDomainForName(domainName)`: `-volatileDomainForName:`
- `volatileDomainNames()`: `-volatileDomainNames`
