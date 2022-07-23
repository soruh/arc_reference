use std::{
    fmt::{Debug, Display},
    ops::Deref,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

macro_rules! implementation {
    ($reference_name: ident, $context_name: ident, $rc_type: ident, $multiple_method_name: ident) => {
        pub struct $reference_name<O, R>
        where
            R: ?Sized,
        {
            inner: $rc_type<O>,
            ptr: NonNull<R>,
        }

        impl<O, R> $reference_name<O, R>
        where
            R: ?Sized,
        {
            pub fn new(inner: $rc_type<O>, f: impl FnOnce(&O) -> &R) -> Self {
                unsafe {
                    Self {
                        ptr: NonNull::new_unchecked(f(&inner) as *const R as *mut R),
                        inner,
                    }
                }
            }

            pub fn source(&self) -> &$rc_type<O> {
                &self.inner
            }
        }

        impl<O, R> Clone for $reference_name<O, R>
        where
            R: ?Sized,
        {
            fn clone(&self) -> Self {
                Self {
                    inner: self.inner.clone(),
                    ptr: self.ptr,
                }
            }
        }

        impl<O, R> Deref for $reference_name<O, R>
        where
            R: ?Sized,
        {
            type Target = R;

            fn deref(&self) -> &Self::Target {
                unsafe { &*self.ptr.as_ptr() }
            }
        }
        impl<O, R> AsRef<R> for $reference_name<O, R>
        where
            R: ?Sized,
        {
            fn as_ref(&self) -> &R {
                &*self
            }
        }

        impl<O, R> Display for $reference_name<O, R>
        where
            R: ?Sized + Display,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                <R as Display>::fmt(&self, f)
            }
        }

        impl<O, R> Debug for $reference_name<O, R>
        where
            R: ?Sized + Debug,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                <R as Debug>::fmt(&self, f)
            }
        }

        pub struct $context_name<'a, T> {
            inner: &'a $rc_type<T>,
        }

        impl<'a, O> $context_name<'a, O> {
            pub fn new_reference<R>(&'a self, r: &'a R) -> $reference_name<O, R> {
                unsafe {
                    $reference_name {
                        ptr: NonNull::new_unchecked(r as *const R as *mut R),
                        inner: self.inner.clone(),
                    }
                }
            }
        }

        pub fn $multiple_method_name<T, R>(
            arc: &$rc_type<T>,
            f: impl FnOnce($context_name<T>, &T) -> R,
        ) -> R {
            f($context_name { inner: &arc }, &arc)
        }
    };
}

implementation!(RcReference, RcMultipleContext, Rc, rc_multiple);
implementation!(ArcReference, ArcMultipleContext, Arc, arc_multiple);

unsafe impl<O, R> Send for ArcReference<O, R>
where
    R: ?Sized,
    Arc<O>: Send,
    for<'r> &'r R: Send,
{
}

unsafe impl<O, R> Sync for ArcReference<O, R>
where
    R: ?Sized,
    Arc<O>: Sync,
    for<'r> &'r R: Sync,
{
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;

    #[test]
    fn rc() {
        let rc = Rc::new(String::from("Hello World!"));

        let hello = RcReference::new(rc.clone(), |string| &string[0..5]);
        let world = RcReference::new(rc.clone(), |string| &string[6..11]);

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn arc() {
        let arc = Arc::new(String::from("Hello World!"));

        let hello = ArcReference::new(arc.clone(), |string| &string[0..5]);
        let world = ArcReference::new(arc.clone(), |string| &string[6..11]);

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn drop_arc() {
        let arc = Arc::new(String::from("Hello World!"));

        let hello = ArcReference::new(arc.clone(), |string| &string[0..5]);
        let world = ArcReference::new(arc.clone(), |string| &string[6..11]);

        drop(arc);

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn access_source() {
        let rc = Rc::new(String::from("Hello World!"));

        let hello = RcReference::new(rc.clone(), |string| &string[0..5]);

        drop(rc);

        let world = RcReference::new(hello.source().clone(), |string| &string[6..11]);

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn threaded() {
        macro_rules! with_cloned {
            ([$($variables: ident),* $(,)?], $body: stmt) => {
                {
                    $(let $variables = $variables.clone();)*

                    $body
                }
            };
        }

        let arc = Arc::new(String::from("Hello World!"));

        let hello = ArcReference::new(arc.clone(), |string| &string[0..5]);
        let world = ArcReference::new(arc.clone(), |string| &string[6..11]);

        let barrier = Arc::new(Barrier::new(3));

        let a = with_cloned!(
            [barrier, hello, world],
            std::thread::spawn(move || {
                barrier.wait();
                assert_eq!(format!("{hello} {world}"), "Hello World");
            })
        );

        let b = with_cloned!(
            [barrier, hello, world],
            std::thread::spawn(move || {
                barrier.wait();
                assert_eq!(format!("{world} {hello}"), "World Hello");
            })
        );

        let c = std::thread::spawn(move || {
            drop(arc);
            barrier.wait();
        });

        a.join().unwrap();
        b.join().unwrap();
        c.join().unwrap();
    }

    #[test]
    fn test_multiple_arc() {
        struct Foo {
            a: u8,
            b: u32,
            c: String,
        }

        let foo = Arc::new(Foo {
            a: 42,
            b: 1024,
            c: String::from("Foo"),
        });

        let (a, b, c) = arc_multiple(&foo, |ctx, value| {
            (
                ctx.new_reference(&value.a),
                ctx.new_reference(&value.b),
                ctx.new_reference(&value.c),
            )
        });

        drop(foo);

        let a = std::thread::spawn(move || {
            assert_eq!(*a, 42);
        });
        let b = std::thread::spawn(move || {
            assert_eq!(*b, 1024);
        });
        let c = std::thread::spawn(move || {
            assert_eq!(*c, "Foo");
        });

        a.join().unwrap();
        b.join().unwrap();
        c.join().unwrap();
    }

    #[test]
    fn test_multiple_rc() {
        struct Foo {
            a: u8,
            b: u32,
            c: String,
        }

        let foo = Rc::new(Foo {
            a: 42,
            b: 1024,
            c: String::from("Foo"),
        });

        let (a, b, c) = rc_multiple(&foo, |ctx, value| {
            (
                ctx.new_reference(&value.a),
                ctx.new_reference(&value.b),
                ctx.new_reference(&value.c),
            )
        });

        drop(foo);

        assert_eq!(*a, 42);
        assert_eq!(*b, 1024);
        assert_eq!(*c, "Foo");
    }
}
