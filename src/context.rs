use std::{
    any::{type_name, TypeId},
    marker::PhantomData,
    ptr::NonNull,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use hecs::TypeInfo;

use crate::{Error, Result};

/// Holds all data necessary for the execution of the world
pub struct Context<'a> {
    data: &'a dyn Data,
}

impl<'a> Context<'a> {
    /// Construct a new context from the tuple of references `data`
    pub fn new(data: &'a dyn Data) -> Context {
        Self { data }
    }

    /// Borrows data of type T mutably from the context. Does not panic.
    pub fn borrow<T: 'static>(&self) -> Result<AtomicRef<T>> {
        let val = unsafe { self.data.get(TypeId::of::<T>()) }
            .ok_or_else(|| Error::MissingData(TypeInfo::of::<T>()))?;

        let val = val
            .try_borrow()
            .map_err(|_| Error::Borrow(type_name::<T>()))?;

        Ok(AtomicRef::map(val, |val| unsafe {
            val.cast::<T>().as_ref()
        }))
    }

    /// Borrows data of type T mutably from the context. Does not panic.
    pub fn borrow_mut<T: 'static>(&self) -> Result<AtomicRefMut<T>> {
        let val = unsafe { self.data.get(TypeId::of::<T>()) }
            .ok_or_else(|| Error::MissingData(TypeInfo::of::<T>()))?;

        let val = val
            .try_borrow_mut()
            .map_err(|_| Error::BorrowMut(type_name::<T>()))?;

        Ok(AtomicRefMut::map(val, |val| unsafe {
            val.cast::<T>().as_mut()
        }))
    }
}

pub trait Data {
    unsafe fn get<'a>(&'a self, ty: TypeId) -> Option<&AtomicRefCell<NonNull<u8>>>;
}

impl<A: 'static> Data for (AtomicRefCell<NonNull<u8>>, PhantomData<A>) {
    unsafe fn get<'a>(&'a self, ty: TypeId) -> Option<&AtomicRefCell<NonNull<u8>>> {
        if ty == TypeId::of::<A>() {
            Some(&self.0)
        } else {
            None
        }
    }
}

trait IntoData {
    type Target: Data;
    unsafe fn into_data(self) -> Self::Target;
}

macro_rules! tuple_impls {
    () => {};
    (($idx:tt => $typ:ident), $( ($nidx:tt => $ntyp:ident), )*) => {
        /*
         * Invoke recursive reversal of list that ends in the macro expansion implementation
         * of the reversed list
        */
        tuple_impls!([($idx, $typ);] $( ($nidx => $ntyp), )*);
        tuple_impls!($( ($nidx => $ntyp), )*); // invoke macro on tail
    };

        /*
     * ([accumulatedList], listToReverse); recursively calls tuple_impls until the list to reverse
     + is empty (see next pattern)
    */
    ([$(($accIdx: tt, $accTyp: ident);)+]  ($idx:tt => $typ:ident), $( ($nidx:tt => $ntyp:ident), )*) => {
      tuple_impls!([($idx, $typ); $(($accIdx, $accTyp); )*] $( ($nidx => $ntyp), ) *);
    };

    // Finally expand into the implementation
    ([($idx:tt, $typ:ident); $( ($nidx:tt, $ntyp:ident); )*]) => {
        impl<$typ, $( $ntyp ), *> IntoData for (&mut $typ, $(&mut $ntyp,) *)
            where
                $typ: 'static,
                $($ntyp: 'static), *
        {
            type Target = ((AtomicRefCell<NonNull<u8>>, PhantomData<$typ>), $( (AtomicRefCell<NonNull<u8>>, PhantomData<$ntyp>), )*);

            unsafe fn into_data(self) -> Self::Target {
                (
                    (AtomicRefCell::new(NonNull::new_unchecked(self.$idx as *mut _ as *mut u8)), PhantomData),
                    $( (AtomicRefCell::new(NonNull::new_unchecked(self.$nidx as *mut _ as *mut u8)), PhantomData), ) *
                )
            }

        }

        impl<$typ, $( $ntyp ), *> Data for ((AtomicRefCell<NonNull<u8>>, PhantomData<$typ>), $( (AtomicRefCell<NonNull<u8>>, PhantomData<$ntyp>), )*)
            where
                $typ: 'static,
                $($ntyp: 'static), *

        {
            unsafe fn get<'a>(&'a self, ty: TypeId) -> Option<&AtomicRefCell<NonNull<u8>>> {
                if ty == TypeId::of::<$typ>() {
                    Some(&self.$idx.0)
                } $(else if ty == TypeId::of::<$ntyp>() {
                    Some(&self.$nidx.0)
                }) *
                else {
                    None
                }
            }
        }
    };
}

tuple_impls!(
    (9 => J),
    (8 => I),
    (7 => H),
    (6 => G),
    (5 => F),
    (4 => E),
    (3 => D),
    (2 => C),
    (1 => B),
    (0 => A),
);

#[cfg(test)]
mod tests {
    use super::{Context, IntoData};

    #[test]
    fn context() {
        let mut a = 64_i32;
        let mut b = "Hello, World";

        let data = unsafe { (&mut a, &mut b).into_data() };

        let context = Context::new(&data);

        {
            let a = context.borrow::<i32>().unwrap();
            let mut b = context.borrow_mut::<&str>().unwrap();

            assert_eq!(*b, "Hello, World");
            *b = "Foo Fighters";
            drop(b);

            let b = context.borrow::<&str>().unwrap();
            assert_eq!(*a, 64);
            assert_eq!(*b, "Foo Fighters");

            let c = context.borrow::<f32>();
            assert!(c.is_err());
        }
    }
}