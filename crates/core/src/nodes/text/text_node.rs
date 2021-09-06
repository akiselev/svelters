use rquickjs::{bind, HasRefs, Object, Persistent};

use crate::inner::*;

#[derive(Clone, Eq, PartialEq)]
pub struct TextNode<'js>(pub(crate) Inner<TextData<'js>>);

impl<'js> std::ops::Deref for TextNode<'js> {
    type Target = Inner<TextData<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> TextNode<'js> {
    // constructor
    pub fn new(text: String) -> Self {
        Self(Inner::new(TextData { text, node: NodeData::new() }))
    }

    pub fn get_text(&self) -> String {
        self.with(|data| data.text.clone())
    }

    pub fn set_text(&mut self, text: String) {
        self.with_mut(|data| data.text = text)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TextData<'js> {
    pub(in crate) text: String, 
    pub(in crate) node: NodeData<'js>,
}
