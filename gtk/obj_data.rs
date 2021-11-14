use crate::common::*;

type Result<T, E> = std::result::Result<T, E>;

pub trait ObjectData {
    fn get_data<T>(&self, name: &str) -> Option<&mut T>
    where
        T: 'static;

    fn get_or_create_data<T, FN>(&self, name: &str, func: FN) -> &mut T
    where
        T: 'static,
        FN: FnOnce() -> T;

    fn try_get_or_create_data<T, E, FN>(&self, name: &str, func: FN) -> Result<&mut T, E>
    where
        T: 'static,
        FN: FnOnce() -> Result<T, E>;
}

impl ObjectData for GObject {
    fn get_data<T>(&self, name: &str) -> Option<&mut T>
    where
        T: 'static,
    {
        unsafe { self.data::<T>(name).map(|mut inner| inner.as_mut()) }
    }

    fn get_or_create_data<T, FN>(&self, name: &str, func: FN) -> &mut T
    where
        T: 'static,
        FN: FnOnce() -> T,
    {
        unsafe {
            if let Some(mut data) = self.data::<T>(name) {
                data.as_mut()
            } else {
                let value = func();
                self.set_data(name, value);
                self.data::<T>(name).unwrap().as_mut()
            }
        }
    }

    fn try_get_or_create_data<T, E, FN>(&self, name: &str, func: FN) -> Result<&mut T, E>
    where
        T: 'static,
        FN: FnOnce() -> Result<T, E>,
    {
        unsafe {
            if let Some(mut data) = self.data::<T>(name) {
                Ok(data.as_mut())
            } else {
                let value = func()?;
                self.set_data(name, value);
                Ok(self.data::<T>(name).unwrap().as_mut())
            }
        }
    }
}
