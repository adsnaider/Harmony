#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;
use core::ops::Deref;

use tailcall::tailcall;

#[derive(Debug)]
#[repr(transparent)]
pub struct TrieEntry<const COUNT: usize, S: Slot<COUNT>> {
    slots: [S; COUNT],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct SlotId<const COUNT: usize>(usize);

impl<const COUNT: usize> SlotId<COUNT> {
    pub fn new(id: usize) -> Result<Self, TrieIndexError> {
        Self::try_from(id)
    }

    pub const fn count() -> usize {
        COUNT
    }

    pub const fn bits() -> usize {
        COUNT.trailing_zeros() as usize
    }
}

impl<const COUNT: usize> TryFrom<usize> for SlotId<COUNT> {
    type Error = TrieIndexError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value < COUNT {
            Ok(Self(value))
        } else {
            Err(TrieIndexError::OutOfBounds)
        }
    }
}

impl<const COUNT: usize> From<SlotId<COUNT>> for usize {
    fn from(value: SlotId<COUNT>) -> Self {
        value.0
    }
}

impl<const COUNT: usize, S: Slot<COUNT> + Default> Default for TrieEntry<COUNT, S> {
    fn default() -> Self {
        Self {
            slots: core::array::from_fn(|_| S::default()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TrieIndexError {
    OutOfBounds,
}

impl<const COUNT: usize, S: Slot<COUNT> + Default> TrieEntry<COUNT, S> {
    pub const fn slot_size() -> usize {
        core::mem::size_of::<S>()
    }

    pub fn index(this: S::Ptr, idx: SlotId<COUNT>) -> impl Ptr<S> {
        this.map(move |entry| unsafe { entry.slots.get_unchecked(idx.0) })
    }

    pub fn get(this: S::Ptr, id: u32) -> Result<Option<impl Ptr<S>>, S::Err> {
        let id = usize::try_from(id).unwrap();
        Self::get_inner(this, id)
    }

    #[tailcall]
    fn get_inner(this: S::Ptr, id: usize) -> Result<Option<impl Ptr<S>>, S::Err> {
        let offset: usize = id % COUNT;
        let id = id / COUNT;
        if id == 0 {
            let node = this.map(move |entry| &entry.slots[offset]);
            Ok(Some(node))
        } else {
            let slot = &this.slots[offset];
            let Some(child) = slot.child()? else {
                return Ok(None);
            };
            Self::get_inner(child, id)
        }
    }
}

pub struct PtrMap<T, P: Ptr<T>, U, F: Fn(&T) -> &U> {
    ptr: P,
    fun: F,
    _t: PhantomData<T>,
    _u: PhantomData<U>,
}

pub trait Ptr<T>: Deref<Target = T> + Sized {
    fn map<U, F>(self, fun: F) -> impl Ptr<U>
    where
        F: Fn(&T) -> &U,
    {
        PtrMap {
            ptr: self,
            fun,
            _t: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<T, P: Ptr<T>, U, F: Fn(&T) -> &U> Ptr<U> for PtrMap<T, P, U, F> {}
impl<T, P: Ptr<T>, U, F: Fn(&T) -> &U> Deref for PtrMap<T, P, U, F> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        (self.fun)(self.ptr.deref())
    }
}

pub trait Slot<const COUNT: usize>: Sized {
    type Err;
    type Ptr: Ptr<TrieEntry<COUNT, Self>>;

    fn child(&self) -> Result<Option<Self::Ptr>, Self::Err>;
}

#[cfg(test)]
mod tests {

    use core::cell::{Cell, RefCell};
    use core::convert::Infallible;
    use std::rc::Rc;

    use super::*;

    impl<T> Ptr<T> for Rc<T> {}

    #[derive(Default)]
    struct MySlot<const COUNT: usize> {
        child: RefCell<Option<Rc<TrieEntry<COUNT, Self>>>>,
        payload: Cell<u32>,
    }

    impl<const COUNT: usize> Slot<COUNT> for MySlot<COUNT> {
        type Ptr = Rc<TrieEntry<COUNT, Self>>;
        type Err = Infallible;

        fn child(&self) -> Result<Option<Self::Ptr>, Self::Err> {
            Ok(self.child.borrow().clone())
        }
    }

    impl<const COUNT: usize> MySlot<COUNT> {
        fn set_child(
            &self,
            child: Option<<Self as Slot<COUNT>>::Ptr>,
        ) -> Option<<Self as Slot<COUNT>>::Ptr> {
            core::mem::replace(&mut *self.child.borrow_mut(), child)
        }
    }

    #[test]
    fn smoke() {
        type MyTrie = TrieEntry<16, MySlot<16>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());
        assert_eq!(
            MyTrie::get(trie.clone(), 0).unwrap().unwrap().payload.get(),
            0
        );
    }

    #[test]
    fn slots_can_be_set_and_get() {
        type MyTrie = TrieEntry<64, MySlot<64>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());

        for id in 0..64 {
            let slot = MyTrie::get(trie.clone(), id).unwrap().unwrap();
            slot.payload.set(id);
        }

        for id in 0..64 {
            let slot = MyTrie::get(trie.clone(), id).unwrap().unwrap();
            assert_eq!(slot.payload.get(), id);
        }
    }

    #[test]
    fn connections() {
        type MyTrie = TrieEntry<64, MySlot<64>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());

        assert!(MyTrie::get(trie.clone(), 64).unwrap().is_none());
        let slot = MyTrie::get(trie.clone(), 0).unwrap().unwrap();
        let l1: Rc<MyTrie> = Rc::new(TrieEntry::default());
        assert!(slot.set_child(Some(l1)).is_none());
        assert_eq!(
            MyTrie::get(trie.clone(), 64)
                .unwrap()
                .unwrap()
                .payload
                .get(),
            0
        );
    }
    #[test]
    fn connections2() {
        type MyTrie = TrieEntry<64, MySlot<64>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());

        assert!(MyTrie::get(trie.clone(), 64).unwrap().is_none());
        let slot = MyTrie::get(trie.clone(), 0).unwrap().unwrap();
        let l1: Rc<MyTrie> = Rc::new(TrieEntry::default());
        l1.slots[4].payload.set(10);
        assert!(slot.set_child(Some(l1)).is_none());
        assert_eq!(
            MyTrie::get(trie.clone(), 4 << 6)
                .unwrap()
                .unwrap()
                .payload
                .get(),
            10
        );
    }
}
