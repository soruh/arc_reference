use std::{
    fmt::{Debug, Display},
    ops::Deref,
    ptr::NonNull,
    sync::Arc,
};

pub trait TakeArcReference<O> {
    fn take_reference<R>(&self, f: impl FnOnce(&O) -> &R) -> ArcReference<O, R>
    where
        R: ?Sized;
}

impl<O> TakeArcReference<O> for Arc<O> {
    fn take_reference<R>(&self, f: impl FnOnce(&O) -> &R) -> ArcReference<O, R>
    where
        R: ?Sized,
    {
        ArcReference::new(Arc::clone(self), f)
    }

    // fn take_references<'r, R, F, I, J>(&self, f: F) -> J
    // where
    //     R: ToOwned<Owned = T> + ?Sized + 'r,
    //     I: IntoIterator<Item = &'r R>,
    //     F: FnOnce(&'r T) -> I,
    //     T: 'r,
    //     J: Iterator<Item = ArcReference<R>>,
    // {
    //     ArcReference::multiple(Arc::clone(self), f)
    // }
}

pub struct ArcReference<O, R>
where
    R: ?Sized,
{
    inner: Arc<O>,
    ptr: NonNull<R>,
}

impl<O, R> ArcReference<O, R>
where
    R: ?Sized,
{
    pub fn new(inner: Arc<O>, f: impl FnOnce(&O) -> &R) -> Self {
        unsafe {
            Self {
                ptr: NonNull::new_unchecked(f(&inner) as *const R as *mut R),
                inner,
            }
        }
    }
}

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

impl<O, R> Clone for ArcReference<O, R>
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

impl<O, R> Deref for ArcReference<O, R>
where
    R: ?Sized,
{
    type Target = R;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}
impl<O, R> AsRef<R> for ArcReference<O, R>
where
    R: ?Sized,
{
    fn as_ref(&self) -> &R {
        &*self
    }
}

impl<O, R> Display for ArcReference<O, R>
where
    R: ?Sized + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <R as Display>::fmt(&self, f)
    }
}

impl<O, R> Debug for ArcReference<O, R>
where
    R: ?Sized + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <R as Debug>::fmt(&self, f)
    }
}

pub struct Context<'a, T> {
    inner: &'a Arc<T>,
}

impl<'a, O> Context<'a, O> {
    pub fn new_reference<R>(&'a self, r: &'a R) -> ArcReference<O, R> {
        unsafe {
            ArcReference {
                ptr: NonNull::new_unchecked(r as *const R as *mut R),
                inner: self.inner.clone(),
            }
        }
    }
}

pub fn multiple<T, R>(arc: &Arc<T>, f: impl FnOnce(Context<T>, &T) -> R) -> R {
    f(Context { inner: &arc }, &arc)
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;

    macro_rules! with_cloned {
        ([$($variables: ident),* $(,)?], $body: stmt) => {
            {
                $(let $variables = $variables.clone();)*

                $body
            }
        };
    }

    #[test]
    fn basic() {
        let arc = Arc::new(String::from("Hello World!"));

        let hello = ArcReference::new(arc.clone(), |string| -> &str { &string[0..5] });
        let world = ArcReference::new(arc.clone(), |string| -> &str { &string[6..11] });

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn drop_arc() {
        let arc = Arc::new(String::from("Hello World!"));

        let hello = ArcReference::new(arc.clone(), |string| -> &str { &string[0..5] });
        let world = ArcReference::new(arc.clone(), |string| -> &str { &string[6..11] });

        drop(arc);

        assert_eq!(format!("{hello} {world}"), "Hello World");
    }

    #[test]
    fn threaded() {
        let arc = Arc::new(String::from("Hello World!"));

        let hello = arc.take_reference(|string| &string[0..5]);
        let world = arc.take_reference(|string| &string[6..11]);

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
    fn test_multiple() {
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

        let (a, b, c) = multiple(&foo, |ctx, value| {
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
}
