<grid anchor="subject" grid-template-columns="auto 1fr" column-gap={8} row-gap={4} padding={8} align-items="center">
  <label>Reference</label>
  <text-edit text="U3" />
  <label>Footprint</label>
  <combo-box accessible-name="Footprint" options={['SOIC-8', 'QFN-16']} selected={0} />
  <label>Do not populate</label>
  <checkbox accessible-name="Do not populate" />
</grid>
