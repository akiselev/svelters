<!-- kinds: Image ProgressBar ProgressRing InfoBar InfoBadge PersonPicture SymbolIcon FontIcon -->
<stack-panel orientation="vertical" spacing="6">
  <image width={48} height={48} stretch="uniform" />
  <progress-bar maximum={100} value={60} />
  <progress-bar is-indeterminate={true} />
  <progress-ring is-active={true} />
  <info-bar title="Info bar" message="Something happened." severity="success" is-open={true} is-closable={false}>
    <text-block>Extra info bar content</text-block>
  </info-bar>
  <info-badge value={7} />
  <person-picture display-name="Ada Lovelace" />
  <stack-panel orientation="horizontal" spacing="8">
    <symbol-icon symbol="accept" />
    <symbol-icon symbol="save" />
    <font-icon glyph={''} font-size={20} />
  </stack-panel>
</stack-panel>
