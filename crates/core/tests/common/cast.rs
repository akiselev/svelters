use std::{borrow::Borrow, marker::PhantomData, rc::Rc};

use super::*;

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<Object>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<Widget>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<Application>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<MainWindow>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<BoxLayout>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<AbstractButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Object>>> for Fw<Rc<PushButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Object>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Widget>>> for Fw<Rc<Widget>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Widget>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<Widget>>> for Fw<Rc<MainWindow>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Widget>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Widget>>> for Fw<Rc<BoxLayout>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Widget>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Widget>>> for Fw<Rc<AbstractButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Widget>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<Widget>>> for Fw<Rc<PushButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Widget>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<AbstractButton>>> for Fw<Rc<AbstractButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<AbstractButton>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<AbstractButton>>> for Fw<Rc<PushButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<AbstractButton>> {
        self.0 .0.static_cast()
    }
}

impl StaticUpcast<Fw<Rc<PushButton>>> for Fw<Rc<PushButton>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<PushButton>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<BoxLayout>>> for Fw<Rc<BoxLayout>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<BoxLayout>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<MainWindow>>> for Fw<Rc<MainWindow>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<MainWindow>> {
        self.clone()
    }
}

impl StaticUpcast<Fw<Rc<Application>>> for Fw<Rc<Application>> {
    fn static_cast<'a>(&'a self) -> Fw<Rc<Application>> {
        self.clone()
    }
}

// impl Cast<Fw<Rc<Object>>> for Fw<Rc<Object>> {
//     type Output<'a> = RefWrapper<'a, Fw<Rc<Object>>>;

//     fn cast<'a>(&'a self) -> Self::Output<'a> {
//         RefWrapper(self.clone(), PhantomData)
//     }
// }

// impl<T, O> Cast<T> for FwWrapper<O>
// where
//     O: StaticUpcast<T>,
//     T: Clone,
// {
//     type Output<'a> = RefWrapper<'a, T>;

//     fn cast<'a>(&'a self) -> Self::Output<'a> {
//         RefWrapper(self.0.static_cast().clone(), PhantomData)
//     }
// }

impl<T, O> Cast<Fw<T>> for Fw<O>
where
    Fw<O>: StaticUpcast<Fw<T>>,
    T: Clone,
{
    type Output<'a> = RefWrapper<'a, Fw<T>>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        RefWrapper(self.static_cast().clone(), PhantomData)
    }
}
