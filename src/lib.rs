use std::{
    collections::VecDeque,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

type Counter = usize;

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
    source: *mut Publisher<T>,
    counter: Counter,
}

impl<'a, T> Deref for BorrowRead<'a, T> {
    type Target = T;

    fn deref(&'_ self) -> &'_ Self::Target {
        self.obj
    }
}

impl<'a, T> Drop for BorrowRead<'a, T> {
    fn drop(&mut self) {
        unsafe { &mut *self.reader.source }.reader_done(ConsumerInfo {
            unread: self.counter,
            reader: self.reader,
        });
    }
}

struct BorrowWrite<'a, T> {
    obj: &'a mut MaybeUninit<T>,
    writer: &'a mut Publisher<T>,
    newcount: Counter,
    written: bool,
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
        if !self.written {
            todo!("cleanup");
        }
    }
}

impl<'a, T> BorrowWrite<'a, T> {
    pub fn finish(mut self) {
        for i in self.writer.readers.iter_mut() {
            unsafe { &mut *i.reader }.new_data(self.obj, self.newcount);
        }
        self.written = true;
    }
}

struct StreamReader<T> {
    phantom: PhantomData<T>,
    source: *mut Publisher<T>,
    unread_data: VecDeque<(*const T, Counter)>,
}

impl<T> StreamReader<T> {
    fn new_data(&mut self, data: &MaybeUninit<T>, count: Counter) {
        self.unread_data.push_back((data.as_ptr(), count));
    }
    fn read(&mut self) -> Option<BorrowRead<'_, T>> {
        let data = self.unread_data.pop_front();
        data.map(|(ptr, counter)| BorrowRead {
            source: self.source,
            obj: unsafe { &*ptr },
            reader: self,
            counter,
        })
    }
    fn new() -> Self {
        Self {
            phantom: PhantomData,
            source: core::ptr::null_mut(),
            unread_data: vec![].into(),
        }
    }
}

impl<T> Drop for StreamReader<T> {
    fn drop(&mut self) {
        if !self.source.is_null() {
            unsafe { &mut *self.source }.remove_reader(self);
        }
    }
}

// aka StreamWriter
pub struct Publisher<T> {
    data: VecDeque<MaybeUninit<T>>,
    first_count: Counter,
    readers: Vec<ConsumerInfo<T>>,
}

impl<T> Publisher<T> {
    pub fn publish(&mut self, obj: T) {
        let newcount = self.first_count.wrapping_add(self.data.len());
        self.data.push_back(MaybeUninit::new(obj));
        if let Some(data) = self.data.back_mut() {
            for i in self.readers.iter_mut() {
                unsafe { &mut *i.reader }.new_data(data, newcount);
            }
        }
    }
    fn add_reader(&mut self, info: ConsumerInfo<T>) {
        let reader = unsafe { &mut *info.reader };
        reader.source = self as *mut _;
        for (n, i) in self.data.iter().enumerate() {
            reader.new_data(i, self.first_count.wrapping_add(n));
        }
        self.readers.push(info);
    }
    fn reader_done(&mut self, info: ConsumerInfo<T>) {
        let mut min_used_minus_first = self.data.len();
        for i in self.readers.iter_mut() {
            if i.reader == info.reader {
                i.unread = info.unread.wrapping_add(1);
            }
            if i.unread.wrapping_sub(self.first_count) < min_used_minus_first {
                min_used_minus_first = i.unread.wrapping_sub(self.first_count);
            }
        }
        if min_used_minus_first > 0 {
            self.first_count += min_used_minus_first;
            for _ in 0..min_used_minus_first {
                self.data.pop_front();
            }
        }
    }
    fn remove_reader(&mut self, rd: &mut StreamReader<T>) {
        let addr = rd as *mut _;
        self.readers.retain(|e| e.reader != addr);
    }

    // if you allocate multiple times, please finish in order
    pub fn allocate(&mut self) -> BorrowWrite<T> {
        let newcount = self.first_count.wrapping_add(self.data.len());
        self.data.push_back(MaybeUninit::uninit());
        let unbound_self_ref = unsafe { &mut *(self as *mut _) };
        BorrowWrite {
            obj: self.data.back_mut().unwrap(),
            writer: unbound_self_ref,
            newcount,
            written: false,
        }
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
