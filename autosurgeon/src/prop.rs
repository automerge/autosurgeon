use automerge as am;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub enum Prop<'a> {
    Key(Cow<'a, str>),
    Index(u32),
}

impl<'a> std::fmt::Display for Prop<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key(s) => write!(f, "{}", s),
            Self::Index(i) => write!(f, "{}", i),
        }
    }
}

impl<'a> From<&Prop<'a>> for automerge::Prop {
    fn from(p: &Prop) -> Self {
        match p {
            Prop::Key(k) => automerge::Prop::Map(k.to_string()),
            Prop::Index(i) => automerge::Prop::Seq(*i as usize),
        }
    }
}

impl From<u32> for Prop<'static> {
    fn from(v: u32) -> Self {
        Prop::Index(v)
    }
}

impl From<usize> for Prop<'static> {
    fn from(v: usize) -> Self {
        Prop::Index(v as u32)
    }
}

impl<'a> From<Cow<'a, str>> for Prop<'a> {
    fn from(v: Cow<'a, str>) -> Self {
        Self::Key(v)
    }
}

impl<'a> From<&'a str> for Prop<'a> {
    fn from(s: &'a str) -> Self {
        Self::Key(s.into())
    }
}

impl<'a> From<am::Prop> for Prop<'a> {
    fn from(p: am::Prop) -> Self {
        match p {
            am::Prop::Map(k) => Prop::Key(Cow::Owned(k)),
            am::Prop::Seq(idx) => Prop::Index(idx as u32),
        }
    }
}

impl<'a> From<&'a am::Prop> for Prop<'a> {
    fn from(p: &'a am::Prop) -> Self {
        match p {
            am::Prop::Map(k) => Prop::Key(Cow::Borrowed(k)),
            am::Prop::Seq(idx) => Prop::Index(*idx as u32),
        }
    }
}
