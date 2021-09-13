use crate::inner::*;

impl<'a, 'js, 'c, BASE, CLS, SLOT> Implements<InitElementProto<'a, 'js, 'c, BASE>>
    for members::NativeSlot<CLS, SLOT>
where
    BASE: Type + Upcast<CLS>,
    CLS: Type,
    SLOT: INativeSlot,
{
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitElementProto<'a, 'js, 'c, BASE>,
    ) -> Result<InitElementProto<'a, 'js, 'c, BASE>> {
        tracing::info!(
            "InitElementProto(BASE: {}, CLS: {})",
            BASE::CLASS_NAME,
            CLS::CLASS_NAME
        );
        {
            let slot = self.slot.clone();
            phase.obj.set(
                "appendChild",
                rquickjs::Func::new(
                    "appendChild",
                    rquickjs::Method(move |this, child| slot.append_child(this, child)),
                ),
            )?;
        }
        {
            let slot = self.slot.clone();
            phase.obj.set(
                "removeChild",
                rquickjs::Func::new(
                    "removeChild",
                    rquickjs::Method(move |this, child| slot.remove_child(this, child)),
                ),
            )?;
        }
        {
            let slot = self.slot.clone();
            phase.obj.set(
                "insertBefore",
                rquickjs::Func::new(
                    "insertBefore",
                    rquickjs::Method(move |this, child, anchor| {
                        slot.insert_before(this, child, anchor)
                    }),
                ),
            )?;
        }
        Ok(phase)
    }
}

fn append_child<'js, SLOT: INativeSlot>(
    slot: &SLOT,
    this: Node<'js>,
    child: Node<'js>,
) -> Result<()> {
    slot.append_child(this, child)
}

fn remove_child<'js, SLOT: INativeSlot>(
    slot: &SLOT,
    this: Node<'js>,
    child: Node<'js>,
) -> Result<()> {
    slot.remove_child(this, child)
}

fn insert_before<'js, SLOT: INativeSlot>(
    slot: &SLOT,
    this: Node<'js>,
    child: Node<'js>,
    anchor: Opt<Node<'js>>,
) -> Result<()> {
    slot.insert_before(this, child, anchor.into_inner())
}
