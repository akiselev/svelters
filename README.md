# svelte-rs

## Desktop Rust apps powered by Svelte... the Javascript framework? It's 2021, just roll with it. 

### What `svelte-rs` is NOT

* **It is not a Javascript framework.** If you're more familiar with transient dependencies breaking your Webpack build than linker errors, `svelte-rs` is probably not for you.
* **It will not rid your world of electron.** Sorry, I can't move tides or convinces product managers that usability is important to users.
* **It is not the end-all-be-all of Rust frontend frameworks.** Despite what I'm going to write in my resume, `svelte-rs` is meant to be a shortcut to put decades of technical debt to good use until we are blessed with The One framework.
* **Well tested.** The vast majority of the code in this library is automatically generated from structured API schemas/documentation with a small layer of helper code to bring the gap between C++ (mostly function overloading and serialize/deserialization) and Rust. The API surface is gargantuan - please help!
* **It is not SAFE!** `svelte-rs` overcompensates for Rust's static safety by combining it with the wildest, most dynamic language in UI development this side of a lisp machine. Figuring out the safety of this library is probably best left to the core Rust team once they have enough experience with fixing `specialization`.


### What `svelte-rs` IS

* **UNSAFE** Each of the frameworks has its own ownership model that might have impedance mismatches with Rust. `svelte-rs` attempts to provide a sane interface using safety checks provided by each UI library but it's a WORK IN PROGRESS.
* **Ownership bypass** 
* **Framework agnostic** `svelte-rs` is a thin bridge between Rust, Svelte, and native UI frameworks.
* **A serious attempt at a Rust GUI library** 

### How does it work

1. `svelte-rs` creates shim `Window` and `Document` objects
1. The compiled Svelte components call `document.createElement`
2. `svelte-rs` maps element names to constructors and returns a wrapper that implements a subset of the DOM Element api like `setAttribute`/`addEventListener`/etc. This wrapper also handles marshalling the types between languages and abstracts over ownership
3. The Svelte components create the elements as if they were in a browser and then mounts them with `insertBefore`/`appendChild`/etc.
4. `svelte-rs` checks the type of 