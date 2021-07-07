use thiserror::Error;

use crate::inner::*;

pub enum Return<T> {
    Noop,
    Value(T),
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Definition error: {0}")]
    DefinitionError(String),
}

pub type Result<T, E = Error> = std::result::Result<Return<T>, E>;

pub trait VM<'cls>: Sized + Copy {
    fn exec<P>(phase: P) -> P::Output<Self>
    where
        P: Phase,
        Self: ExecPhase<P>;
}

pub trait Phase {
    type Input;
    type Output<T>;
}

pub trait ExecPhase<P> {
    fn exec_phase(phase: P::Input) -> <P as Phase>::Output<Self>
    where
        P: Phase,
        Self: Sized;
}
