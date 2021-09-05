use crate::inner::*;

impl<'a, 'js, 'c, CLS, SLOT> Implements<InitElementProto<'a, 'js, 'c, CLS>>
    for members::NativeSlot<CLS, SLOT>
where
    CLS: Type,
    SLOT: NativeSlot,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut InitElementProto<'a, 'js, 'c, CLS>) -> Result<()> {
        phase.obj.set(
            "appendChild",
            rquickjs::Func::new(
                "appendChild",
                rquickjs::Method(
                append_child::<SLOT>
            ))
        )?;
        phase.obj.set(
            "removeChild",
            rquickjs::Func::new(
                "removeChild",
                rquickjs::Method(
                remove_child::<SLOT>
            ))
        )?;
        phase.obj.set(
            "insertBefore",
            rquickjs::Func::new(
                "insertBefore",
                rquickjs::Method(
                insert_before::<SLOT>
            ))
        )?;
        Ok(())
    }
}

fn append_child<'js, SLOT: NativeSlot>(this: Node<'js>, child: Node<'js>) -> Result<()> {
    SLOT::append_child(this, child)
}

fn remove_child<'js, SLOT: NativeSlot>(this: Node<'js>, child: Node<'js>) -> Result<()> {
    SLOT::remove_child(this, child)
}

fn insert_before<'js, SLOT: NativeSlot>(this: Node<'js>, child: Node<'js>, anchor: Node<'js>) -> Result<()> {
    SLOT::insert_before(this, child, anchor)
}
