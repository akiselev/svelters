//! A small arena tree that implements `Element`, built from an indented outline.

#![allow(dead_code)]

use std::borrow::Cow;
use std::num::NonZeroU64;
use svelters_style::{Element, StateMask};

#[derive(Default, Debug)]
pub struct Data {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, String)>,
    pub state: StateMask,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

#[derive(Default, Debug)]
pub struct Tree {
    pub nodes: Vec<Data>,
}

#[derive(Clone, Copy)]
pub struct El<'t> {
    pub tree: &'t Tree,
    pub ix: usize,
}

impl std::fmt::Debug for El<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.data().tag, self.ix)
    }
}

impl<'t> El<'t> {
    fn data(&self) -> &'t Data {
        &self.tree.nodes[self.ix]
    }
    fn sibling(&self, d: isize) -> Option<El<'t>> {
        let p = self.data().parent?;
        let sibs = &self.tree.nodes[p].children;
        let i = sibs.iter().position(|&c| c == self.ix)? as isize + d;
        (i >= 0).then(|| sibs.get(i as usize)).flatten().map(|&ix| El { tree: self.tree, ix })
    }
}

impl Element for El<'_> {
    fn key(&self) -> NonZeroU64 {
        NonZeroU64::new(self.ix as u64 + 1).unwrap()
    }
    fn parent(&self) -> Option<Self> {
        self.data().parent.map(|ix| El { tree: self.tree, ix })
    }
    fn prev_sibling(&self) -> Option<Self> {
        self.sibling(-1)
    }
    fn next_sibling(&self) -> Option<Self> {
        self.sibling(1)
    }
    fn first_child(&self) -> Option<Self> {
        self.data().children.first().map(|&ix| El { tree: self.tree, ix })
    }
    fn tag(&self) -> &str {
        &self.data().tag
    }
    fn id(&self) -> Option<&str> {
        self.data().id.as_deref()
    }
    fn has_class(&self, name: &str) -> bool {
        self.data().classes.iter().any(|c| c == name)
    }
    fn for_each_class(&self, f: &mut dyn FnMut(&str)) {
        for c in &self.data().classes {
            f(c)
        }
    }
    fn attr(&self, name: &str) -> Option<Cow<'_, str>> {
        self.data().attrs.iter().find(|(k, _)| k == name).map(|(_, v)| Cow::Borrowed(v.as_str()))
    }
    fn state(&self) -> StateMask {
        self.data().state
    }
    // HTML's directionality, without `auto`: the nearest `dir` on the element or an ancestor
    fn is_rtl(&self) -> bool {
        let mut e = Some(*self);
        while let Some(x) = e {
            match x.attr("dir").as_deref() {
                Some("rtl") => return true,
                Some("ltr") => return false,
                _ => e = x.parent(),
            }
        }
        false
    }
}

impl Tree {
    /// One node per line, children indented by two spaces:
    /// `tag#id.class1.class2[attr=value][attr2=v] :hovered :focused`.
    pub fn parse(outline: &str) -> Tree {
        let mut t = Tree::default();
        let mut stack: Vec<(usize, usize)> = Vec::new(); // (indent, node)
        for line in outline.lines().filter(|l| !l.trim().is_empty()) {
            let indent = line.len() - line.trim_start().len();
            let mut parts = line.split_whitespace();
            let head = parts.next().unwrap();
            let mut d = Data::default();
            for s in parts {
                let name = s.strip_prefix(':').expect("state like :hovered");
                let bit = StateMask::BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, b)| *b).unwrap_or_else(|| panic!("state {name}"));
                d.state |= bit;
            }
            let (head, attrs) = match head.find('[') {
                Some(i) => (&head[..i], &head[i..]),
                None => (head, ""),
            };
            for a in attrs.split(']').filter(|a| !a.is_empty()) {
                let (k, v) = a.trim_start_matches('[').split_once('=').unwrap();
                d.attrs.push((k.into(), v.into()));
            }
            let mut rest = head;
            let end = rest.find(['#', '.']).unwrap_or(rest.len());
            d.tag = rest[..end].into();
            rest = &rest[end..];
            while !rest.is_empty() {
                let kind = rest.as_bytes()[0];
                let end = rest[1..].find(['#', '.']).map(|i| i + 1).unwrap_or(rest.len());
                let name = rest[1..end].to_string();
                if kind == b'#' {
                    d.id = Some(name);
                } else {
                    d.classes.push(name);
                }
                rest = &rest[end..];
            }
            while stack.last().is_some_and(|(i, _)| *i >= indent) {
                stack.pop();
            }
            let ix = t.nodes.len();
            d.parent = stack.last().map(|(_, p)| *p);
            if let Some(p) = d.parent {
                t.nodes[p].children.push(ix);
            }
            t.nodes.push(d);
            stack.push((indent, ix));
        }
        t
    }

    pub fn el(&self, ix: usize) -> El<'_> {
        El { tree: self, ix }
    }

    /// The first node whose id is `id`.
    pub fn by_id(&self, id: &str) -> El<'_> {
        let ix = self.nodes.iter().position(|d| d.id.as_deref() == Some(id)).unwrap_or_else(|| panic!("no #{id}"));
        El { tree: self, ix }
    }

    pub fn all(&self) -> impl Iterator<Item = El<'_>> {
        (0..self.nodes.len()).map(move |ix| El { tree: self, ix })
    }
}
