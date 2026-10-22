<!-- kinds: TextBox PasswordBox AutoSuggestBox RichEditBox NumberBox Slider RatingControl RadioButtons ComboBox ComboBoxItem -->
<script>
  let name = $state('');
  let amount = $state(3);
  let rating = $state(0);
</script>

<stack-panel orientation="vertical" spacing="6">
  <text-box header="Text box" placeholder-text="Your name" ontext-changed={(t) => (name = t)} />
  <text-block>Hello {name}</text-block>
  <password-box header="Password box" placeholder-text="Secret" />
  <auto-suggest-box header="Auto-suggest box" placeholder-text="Search" />
  <rich-edit-box header="Rich edit box" height={70} />
  <number-box header="Number box" minimum={0} maximum={10} value={amount} spin-button-placement-mode="inline" />
  <slider header="Slider" minimum={0} maximum={100} value={40} />
  <rating-control max-rating={5} caption="Rating control" />
  <radio-buttons header="Radio buttons">
    <radio-button>Small</radio-button>
    <radio-button>Medium</radio-button>
    <radio-button>Large</radio-button>
  </radio-buttons>
  <combo-box header="Combo box" placeholder-text="Pick a fruit">
    <combo-box-item>Apple</combo-box-item>
    <combo-box-item>Banana</combo-box-item>
    <combo-box-item>Cherry</combo-box-item>
  </combo-box>
</stack-panel>
