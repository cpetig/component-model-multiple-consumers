use std::{
    collections::VecDeque,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

type Counter = u32;

struct ConsumerInfo<T> {
    unread: Counter,
    reader: *mut StreamReader<T>,
}

impl<T> ConsumerInfo<T> {
    fn from_reader(r: &mut StreamReader<T>) -> Self {
        Self {
            unread: Default::default(),
            reader: r as *mut StreamReader<T>,
        }
    }
}

struct BorrowRead<'a, T> {
    obj: &'a T,
    reader: &'a mut StreamReader<T>,
}

impl<'a, T> Deref for BorrowRead<'a, T> {
    type Target = T;

    fn deref(&'_ self) -> &'_ Self::Target {
        self.obj
    }
}

impl<'a, T> Drop for BorrowRead<'a, T> {
    fn drop(&mut self) {
        unsafe { &mut *self.reader.source.unwrap() }.reader_done(ConsumerInfo {
            unread: todo!(),
            reader: todo!(),
        });
    }
}

struct BorrowWrite<'a, T> {
    obj: &'a mut MaybeUninit<T>,
    writer: &'a mut Publisher<T>,
}

impl<'a, T> Deref for BorrowWrite<'a, T> {
    type Target = MaybeUninit<T>;

    fn deref(&'_ self) -> &'_ Self::Target {
        self.obj
    }
}

impl<'a, T> DerefMut for BorrowWrite<'a, T> {
    fn deref_mut(&'_ mut self) -> &'_ mut Self::Target {
        self.obj
    }
}

impl<'a, T> Drop for BorrowWrite<'a, T> {
    fn drop(&mut self) {
        todo!()
    }
}

impl<'a, T> BorrowWrite<'a, T> {
    pub fn finish(self) {
        todo!()
    }
}

struct StreamReader<T> {
    phantom: PhantomData<T>,
    source: Option<*mut Publisher<T>>,
    unread_data: VecDeque<*const T>,
}

impl<T> StreamReader<T> {
    fn new_data(&mut self, data: &T) {
        self.unread_data.push_back(data as *const T);
    }
    fn read(&mut self) -> Option<BorrowRead<'_, T>> {
        let data = self.unread_data.pop_front();
        data.map(|ptr| BorrowRead {
            obj: unsafe { &*ptr },
            reader: self,
        })
    }
    fn new() -> Self {
        Self {
            phantom: PhantomData,
            source: None,
            unread_data: vec![].into(),
        }
    }
}

impl<T> Drop for StreamReader<T> {
    fn drop(&mut self) {
        if let Some(pblsh) = self.source {
            unsafe { &mut *pblsh }.remove_reader(self);
        }
    }
}

pub struct Publisher<T> {
    data: VecDeque<T>,
    first_count: Counter,
    readers: Vec<ConsumerInfo<T>>,
}

impl<T> Publisher<T> {
    pub fn publish(&mut self, obj: T) {
        self.data.push_back(obj);
        if let Some(data) = self.data.back_mut() {
            for i in self.readers.iter_mut() {
                unsafe { &mut *i.reader }.new_data(data);
            }
        }
    }
    fn add_reader(&mut self, info: ConsumerInfo<T>) {
        let reader = unsafe { &mut *info.reader };
        reader.source.replace(self as *mut _);
        for i in self.data.iter() {
            reader.new_data(i);
        }
        self.readers.push(info);
    }
    fn reader_done(&mut self, info: ConsumerInfo<T>) {
        todo!()
    }
    fn remove_reader(&mut self, rd: &mut StreamReader<T>) {
        todo!()
    }

    pub fn allocate(&mut self) -> BorrowWrite<T> {
        todo!()
    }

    pub fn new() -> Self {
        Self {
            data: VecDeque::new(),
            first_count: Default::default(),
            readers: vec![],
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{ConsumerInfo, Publisher, StreamReader};
    use std::ops::Deref;

    #[test]
    fn push() {
        // this is not safe, it needs pinning
        let mut p: Publisher<u32> = Publisher::new();
        p.publish(1);
        let mut r1 = StreamReader::new();
        p.add_reader(ConsumerInfo::from_reader(&mut r1));
        assert!(r1.read().unwrap().deref() == &1);
        let mut r2 = StreamReader::new();
        p.add_reader(ConsumerInfo::from_reader(&mut r2));
        let mut w = p.allocate();
        w.write(2);
        w.finish();
        assert!(r2.read().unwrap().deref() == &2);
        assert!(r1.read().unwrap().deref() == &2);
    }
}
