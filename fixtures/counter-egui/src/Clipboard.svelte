<script>
  // DX2: `svelters:clipboard` and `svelters:shell` on egui. crates/svelters-egui/tests/clipboard.rs
  // drives it under kittest with an in-process clipboard and a recording shell.
  import { clipboard } from 'svelters:clipboard';
  import { openPath, revealInFolder } from 'svelters:shell';

  const rows = [
    ['R1', '10k'],
    ['C1', '100n'],
  ];
  let pasted = $state('');
  let refs = $state('none');
  let status = $state('idle');

  async function copy() {
    const tsv = rows.map((r) => r.join('\t')).join('\n');
    const html = `<table>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</table>`;
    await clipboard.write({ 'text/plain': tsv, 'text/html': html, 'application/x-cad-refs+json': rows.map((r) => r[0]) });
    status = 'copied ' + (await clipboard.formats()).length;
  }

  async function pasteRefs() {
    const e = await clipboard.read(['application/x-cad-refs+json', 'application/*', 'text/plain']);
    refs = e ? `${e.type}: ${e.type.endsWith('+json') ? e.json().join(',') : e.text()}` : 'empty';
  }

  async function shell(fn) {
    try {
      await fn((await clipboard.readText()).trim());
      status = 'shell ok';
    } catch (e) {
      status = 'shell error';
    }
  }
</script>

<window title="Clipboard" default-width={420} default-height={300}>
  <box class="column">
    <button onclicked={copy}>Copy table</button>
    <button onclicked={pasteRefs}>Paste refs</button>
    <button onclicked={() => shell(revealInFolder)}>Reveal</button>
    <button onclicked={() => shell(openPath)}>Open</button>
    <text-edit multiline accessible-name="Sheet" bind:text={pasted} />
    <label>Rows: {pasted ? pasted.split('\n').length : 0}, cells: {pasted.split(/\t|\n/).filter(Boolean).join(' ')}</label>
    <label>Refs: {refs}</label>
    <label>Status: {status}</label>
  </box>
</window>

<style>
  .column {
    gap: 8px;
  }
</style>
