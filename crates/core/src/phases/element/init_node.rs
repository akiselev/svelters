use crate::inner::*;

impl<'a, 'js, 'c, CLS, T> Implements<InitElementProto<'a, 'js, 'c, CLS>>
    for members::Element<CLS, T>
where
    CLS: Type,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElementProto<'a, 'js, 'c, CLS>) -> Result<()> {
        phase.obj.prop(
            "nextSibling",
            Accessor::new_get(
                rquickjs::Method(get_next_sibling),
            )
            .enumerable()
        )?;
        
        phase.obj.prop(
            "prevSibling",
            Accessor::new_get(
                rquickjs::Method(get_prev_sibling),
            )
            .enumerable()
        )?;

        phase.obj.prop(
            "parentNode",
            Accessor::new_get(
                rquickjs::Method(get_parent),
            )
            .enumerable()
        )?;

        phase.obj.prop(
            "children",
            Accessor::new_get(
                rquickjs::Method(get_children),
            )
            .enumerable()
        )?;

        Ok(())
    }
}

fn get_next_sibling<'js>(this: Element<'js>) -> Option<Node<'js>> {
    this.borrow().node.next.clone()
}

fn get_prev_sibling<'js>(this: Element<'js>) -> Option<Node<'js>> {
    this.borrow().node.prev.clone()
}

fn get_parent<'js>(this: Element<'js>) -> Option<Node<'js>> {
    this.borrow().node.parent.clone()
}

fn get_children<'js>(this: Element<'js>) -> Option<Vec<Node<'js>>> {
    this.borrow().node.children.clone()
}