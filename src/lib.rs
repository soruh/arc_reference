use std::{
    fmt::{Debug, Display},
    ops::Deref,
    ptr::NonNull,
    sync::Arc,
};

pub trait TakeArcReference<T> {
    fn take_reference<R>(&self, f: impl FnOnce(&T) -> &R) -> ArcReference<R>
    where
        R: ToOwned<Owned = T> + ?Sized;
}

impl<T> TakeArcReference<T> for Arc<T> {
    fn take_reference<R>(&self, f: impl FnOnce(&T) -> &R) -> ArcReference<R>
    where
        R: ToOwned<Owned = T> + ?Sized,
    {
        ArcReference::new(Arc::clone(self), f)
    }
}

pub struct ArcReference<T>
where
    T: ToOwned + ?Sized,
{
    inner: Arc<T::Owned>,
    ptr: NonNull<T>,
}

impl<T> ArcReference<T>
where
    T: ToOwned + ?Sized,
{
    pub fn new(inner: Arc<T::Owned>, f: impl FnOnce(&T::Owned) -> &T) -> Self {
        unsafe {
            Self {
                ptr: NonNull::new_unchecked(f(&inner) as *const _ as *mut _),
                inner,
            }
        }
    }
}

unsafe impl<T> Send for ArcReference<T>
where
    T: ToOwned + ?Sized,
    Arc<T::Owned>: Send,
    for<'r> &'r T: Send,
{
}

unsafe impl<T> Sync for ArcReference<T>
where
    T: ToOwned + ?Sized,
    Arc<T::Owned>: Sync,
    for<'r> &'r T: Sync,
{
}

impl<T> Clone for ArcReference<T>
where
    T: ToOwned + ?Sized,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            ptr: self.ptr,
        }
    }
}

impl<T> Deref for ArcReference<T>
where
    T: ToOwned + ?Sized,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<T> AsRef<T> for ArcReference<T>
where
    T: ToOwned + ?Sized,
{
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T> Display for ArcReference<T>
where
    T: ToOwned + ?Sized + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <T as Display>::fmt(&self, f)
    }
}

impl<T> Debug for ArcReference<T>
where
    T: ToOwned + ?Sized + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <T as Debug>::fmt(&self, f)
    }
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
}
