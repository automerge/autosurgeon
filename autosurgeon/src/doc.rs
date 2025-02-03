use std::ops::RangeBounds;

use automerge::{self as am, AutomergeError, ObjId, Value};

/// An abstraction over the different ways of reading an automerge document
pub trait ReadDoc {
    type Parents<'a>: Iterator<Item = am::Parent>
    where
        Self: 'a;
    fn get_heads(&self) -> Vec<automerge::ChangeHash>;

    fn get<P: Into<am::Prop>>(
        &self,
        obj: &ObjId,
        prop: P,
    ) -> Result<Option<(Value<'_>, ObjId)>, AutomergeError>;

    fn object_type<O: AsRef<ObjId>>(&self, obj: O) -> Option<am::ObjType>;
    fn map_range<'a, O, R>(&'a self, obj: O, range: R) -> am::iter::MapRange<'a, R>
    where
        R: RangeBounds<String> + 'a,
        O: AsRef<ObjId>,
        R: RangeBounds<String>;

    fn list_range<O: AsRef<ObjId>, R: RangeBounds<usize>>(
        &self,
        obj: O,
        range: R,
    ) -> am::iter::ListRange<'_, R>;

    fn length<O: AsRef<ObjId>>(&self, obj: O) -> usize;

    fn text<O: AsRef<ObjId>>(&self, obj: O) -> Result<String, AutomergeError>;
    fn parents<O: AsRef<ObjId>>(&self, obj: O) -> Result<Self::Parents<'_>, AutomergeError>;
}

/// An abstraction over the read + write operations we need from an automerge document
pub trait Doc: ReadDoc {
    fn put<O: AsRef<ObjId>, P: Into<am::Prop>, V: Into<am::ScalarValue>>(
        &mut self,
        obj: O,
        prop: P,
        value: V,
    ) -> Result<(), AutomergeError>;

    fn put_object<O: AsRef<am::ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
        value: am::ObjType,
    ) -> Result<ObjId, AutomergeError>;

    fn insert<O: AsRef<ObjId>, V: Into<am::ScalarValue>>(
        &mut self,
        obj: O,
        index: usize,
        value: V,
    ) -> Result<(), AutomergeError>;

    fn insert_object<O: AsRef<ObjId>>(
        &mut self,
        obj: O,
        index: usize,
        value: am::ObjType,
    ) -> Result<ObjId, AutomergeError>;

    fn increment<O: AsRef<ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
        value: i64,
    ) -> Result<(), AutomergeError>;

    fn delete<O: AsRef<ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
    ) -> Result<(), AutomergeError>;
    fn splice_text<O: AsRef<ObjId>>(
        &mut self,
        obj: O,
        pos: usize,
        del: isize,
        text: &str,
    ) -> Result<(), AutomergeError>;
}

impl ReadDoc for am::AutoCommit {
    type Parents<'a> = am::Parents<'a>;
    fn get_heads(&self) -> Vec<am::ChangeHash> {
        am::transaction::Transactable::base_heads(self)
    }

    fn get<P: Into<am::Prop>>(
        &self,
        obj: &ObjId,
        prop: P,
    ) -> Result<Option<(Value<'_>, ObjId)>, AutomergeError> {
        am::ReadDoc::get(self, obj, prop)
    }

    fn object_type<O: AsRef<ObjId>>(&self, obj: O) -> Option<am::ObjType> {
        am::ReadDoc::object_type(self, obj)
            .map(Some)
            .unwrap_or(None)
    }

    fn map_range<'a, O, R>(&'a self, obj: O, range: R) -> am::iter::MapRange<'a, R>
    where
        R: RangeBounds<String> + 'a,
        O: AsRef<ObjId>,
        R: RangeBounds<String>,
    {
        am::ReadDoc::map_range(self, obj, range)
    }

    fn list_range<O: AsRef<ObjId>, R: RangeBounds<usize>>(
        &self,
        obj: O,
        range: R,
    ) -> am::iter::ListRange<'_, R> {
        am::ReadDoc::list_range(self, obj, range)
    }

    fn length<O: AsRef<ObjId>>(&self, obj: O) -> usize {
        am::ReadDoc::length(self, obj)
    }

    fn text<O: AsRef<ObjId>>(&self, obj: O) -> Result<String, AutomergeError> {
        am::ReadDoc::text(self, obj)
    }

    fn parents<O: AsRef<ObjId>>(&self, obj: O) -> Result<Self::Parents<'_>, AutomergeError> {
        am::ReadDoc::parents(self, obj)
    }
}

impl ReadDoc for am::transaction::Transaction<'_> {
    type Parents<'b>
        = am::Parents<'b>
    where
        Self: 'b;
    fn get_heads(&self) -> Vec<am::ChangeHash> {
        am::transaction::Transactable::base_heads(self)
    }

    fn get<P: Into<am::Prop>>(
        &self,
        obj: &ObjId,
        prop: P,
    ) -> Result<Option<(Value<'_>, ObjId)>, AutomergeError> {
        am::ReadDoc::get(self, obj, prop)
    }

    fn object_type<O: AsRef<ObjId>>(&self, obj: O) -> Option<am::ObjType> {
        am::ReadDoc::object_type(self, obj)
            .map(Some)
            .unwrap_or(None)
    }

    fn map_range<'b, O: AsRef<ObjId>, R>(&'b self, obj: O, range: R) -> am::iter::MapRange<'b, R>
    where
        R: RangeBounds<String> + 'b,
    {
        am::ReadDoc::map_range(self, obj, range)
    }

    fn list_range<O: AsRef<ObjId>, R: RangeBounds<usize>>(
        &self,
        obj: O,
        range: R,
    ) -> am::iter::ListRange<'_, R> {
        am::ReadDoc::list_range(self, obj, range)
    }

    fn length<O: AsRef<ObjId>>(&self, obj: O) -> usize {
        am::ReadDoc::length(self, obj)
    }

    fn text<O: AsRef<ObjId>>(&self, obj: O) -> Result<String, AutomergeError> {
        am::ReadDoc::text(self, obj)
    }

    fn parents<O: AsRef<ObjId>>(&self, obj: O) -> Result<Self::Parents<'_>, AutomergeError> {
        am::ReadDoc::parents(self, obj)
    }
}

impl ReadDoc for am::Automerge {
    type Parents<'a> = am::Parents<'a>;
    fn get_heads(&self) -> Vec<am::ChangeHash> {
        am::Automerge::get_heads(self)
    }

    fn get<P: Into<am::Prop>>(
        &self,
        obj: &ObjId,
        prop: P,
    ) -> Result<Option<(Value<'_>, ObjId)>, AutomergeError> {
        am::ReadDoc::get(self, obj, prop)
    }

    fn object_type<O: AsRef<ObjId>>(&self, obj: O) -> Option<am::ObjType> {
        am::ReadDoc::object_type(self, obj)
            .map(Some)
            .unwrap_or(None)
    }

    fn map_range<'a, O: AsRef<ObjId>, R: RangeBounds<String> + 'a>(
        &'a self,
        obj: O,
        range: R,
    ) -> am::iter::MapRange<'a, R> {
        am::ReadDoc::map_range(self, obj, range)
    }

    fn list_range<O: AsRef<ObjId>, R: RangeBounds<usize>>(
        &self,
        obj: O,
        range: R,
    ) -> am::iter::ListRange<'_, R> {
        am::ReadDoc::list_range(self, obj, range)
    }

    fn length<O: AsRef<ObjId>>(&self, obj: O) -> usize {
        am::ReadDoc::length(self, obj)
    }

    fn text<O: AsRef<ObjId>>(&self, obj: O) -> Result<String, AutomergeError> {
        am::ReadDoc::text(self, obj)
    }

    fn parents<O: AsRef<ObjId>>(&self, obj: O) -> Result<Self::Parents<'_>, AutomergeError> {
        am::ReadDoc::parents(self, obj)
    }
}

impl<T: am::transaction::Transactable + ReadDoc> Doc for T {
    fn put<O: AsRef<ObjId>, P: Into<am::Prop>, V: Into<am::ScalarValue>>(
        &mut self,
        obj: O,
        prop: P,
        value: V,
    ) -> Result<(), AutomergeError> {
        am::transaction::Transactable::put(self, obj, prop, value)
    }

    fn put_object<O: AsRef<am::ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
        value: am::ObjType,
    ) -> Result<ObjId, AutomergeError> {
        am::transaction::Transactable::put_object(self, obj, prop, value)
    }

    fn insert<O: AsRef<ObjId>, V: Into<am::ScalarValue>>(
        &mut self,
        obj: O,
        index: usize,
        value: V,
    ) -> Result<(), AutomergeError> {
        am::transaction::Transactable::insert(self, obj, index, value)
    }

    fn insert_object<O: AsRef<ObjId>>(
        &mut self,
        obj: O,
        index: usize,
        value: am::ObjType,
    ) -> Result<ObjId, AutomergeError> {
        am::transaction::Transactable::insert_object(self, obj, index, value)
    }

    fn increment<O: AsRef<ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
        value: i64,
    ) -> Result<(), AutomergeError> {
        am::transaction::Transactable::increment(self, obj, prop, value)
    }

    fn delete<O: AsRef<ObjId>, P: Into<am::Prop>>(
        &mut self,
        obj: O,
        prop: P,
    ) -> Result<(), AutomergeError> {
        am::transaction::Transactable::delete(self, obj, prop)
    }

    fn splice_text<O: AsRef<ObjId>>(
        &mut self,
        obj: O,
        pos: usize,
        del: isize,
        text: &str,
    ) -> Result<(), AutomergeError> {
        am::transaction::Transactable::splice_text(self, obj, pos, del, text)
    }
}
