#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;
use core::ops::Deref;

use tailcall::tailcall;

#[derive(Debug)]
#[repr(transparent)]
pub struct TrieEntry<const COUNT: usize, S: Slot<COUNT>> {
    slots: [S; COUNT],
}

impl<const COUNT: usize, S: Slot<COUNT> + Default> Default for TrieEntry<COUNT, S> {
    fn default() -> Self {
        Self {
            slots: core::array::from_fn(|_| S::default()),
        }
    }
}

impl<const COUNT: usize, S: Slot<COUNT> + Default> TrieEntry<COUNT, S> {
    pub const fn slot_size() -> usize {
        core::mem::size_of::<S>()
    }

    pub fn get(this: S::Ptr, id: u32) -> Option<impl Ptr<S>> {
        let id = usize::try_from(id).unwrap();
        Self::get_inner(this, id)
    }

    #[tailcall]
    fn get_inner(this: S::Ptr, id: usize) -> Option<impl Ptr<S>> {
        let offset: usize = id % COUNT;
        let id = id / COUNT;
        if id == 0 {
            let node = this.map(move |entry| &entry.slots[offset]);
            Some(node)
        } else {
            let slot = &this.slots[offset];
            let Some(child) = slot.child() else {
                return None;
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
    type Ptr: Ptr<TrieEntry<COUNT, Self>>;

    fn child(&self) -> Option<Self::Ptr>;
    fn set_child(&self, child: Option<Self::Ptr>) -> Option<Self::Ptr>;

    fn link(&self, child: Self::Ptr) -> Option<Self::Ptr> {
        self.set_child(Some(child))
    }

    fn unlink(&self) -> Option<Self::Ptr> {
        self.set_child(None)
    }
}

#[cfg(test)]
mod tests {

    use core::cell::{Cell, RefCell};
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

        fn child(&self) -> Option<Self::Ptr> {
            self.child.borrow().clone()
        }

        fn set_child(&self, child: Option<Self::Ptr>) -> Option<Self::Ptr> {
            core::mem::replace(&mut *self.child.borrow_mut(), child)
        }
    }

    #[test]
    fn smoke() {
        type MyTrie = TrieEntry<16, MySlot<16>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());
        assert_eq!(MyTrie::get(trie.clone(), 0).unwrap().payload.get(), 0);
    }

    #[test]
    fn slots_can_be_set_and_get() {
        type MyTrie = TrieEntry<64, MySlot<64>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());

        for id in 0..64 {
            let slot = MyTrie::get(trie.clone(), id).unwrap();
            slot.payload.set(id);
        }

        for id in 0..64 {
            let slot = MyTrie::get(trie.clone(), id).unwrap();
            assert_eq!(slot.payload.get(), id);
        }
    }

    #[test]
    fn connections() {
        type MyTrie = TrieEntry<64, MySlot<64>>;
        let trie: Rc<MyTrie> = Rc::new(TrieEntry::default());

        assert!(MyTrie::get(trie.clone(), 64).is_none());
        let slot = MyTrie::get(trie.clone(), 0).unwrap();
        let l1: Rc<MyTrie> = Rc::new(TrieEntry::default());
        assert!(slot.link(l1).is_none());
        assert_eq!(MyTrie::get(trie.clone(), 64).unwrap().payload.get(), 0);
    }
}
